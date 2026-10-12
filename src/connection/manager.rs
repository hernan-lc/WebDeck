//! A separately managed LAN listener keeps the local settings connection alive.
use crate::{
    contracts::{ErrorCode, NetworkSettings, NetworkStatus},
    domain::{Error, Result},
    server::{router, App},
};
use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};
use tokio::{sync::Mutex, task::JoinHandle};

type ListenerTask = (JoinHandle<()>, tokio::sync::oneshot::Sender<()>);

pub struct Network {
    path: PathBuf,
    primary_port: u16,
    external: bool,
    enabled: AtomicBool,
    automatic: AtomicBool,
    monitor: Mutex<Option<JoinHandle<()>>>,
    state: Mutex<(NetworkSettings, Option<ListenerTask>)>,
}

use super::discovery::suggested_address;
impl Network {
    pub fn open(path: PathBuf, port: u16, external: bool) -> Result<Self> {
        let settings = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| {
                Error::new(
                    ErrorCode::PersistenceFailed,
                    "Cannot read phone access settings",
                )
            })?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => NetworkSettings {
                enabled: external,
                address: suggested_address(),
                port: port.into(),
            },
            Err(_) => {
                return Err(Error::new(
                    ErrorCode::PersistenceFailed,
                    "Cannot read phone access settings",
                ))
            }
        };
        Ok(Self {
            path,
            primary_port: port,
            external,
            enabled: AtomicBool::new(false),
            automatic: AtomicBool::new(false),
            monitor: Mutex::new(None),
            state: Mutex::new((settings, None)),
        })
    }
    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }
    pub async fn settings(&self) -> NetworkSettings {
        self.state.lock().await.0.clone()
    }
    pub async fn status(&self) -> NetworkStatus {
        let suggested = tokio::task::spawn_blocking(suggested_address)
            .await
            .unwrap_or_default();
        let state = self.state.lock().await;
        let mut settings = state.0.clone();
        settings.enabled = self.enabled();
        drop(state);
        let url = if settings.enabled {
            format!(
                "http://{}/",
                SocketAddr::new(
                    settings.address.parse().expect("validated address"),
                    settings.port as u16
                )
            )
        } else {
            String::new()
        };
        NetworkStatus {
            api_version: 2,
            settings,
            suggested_address: suggested,
            url,
        }
    }
    pub async fn start(&self, app: App) -> Result<()> {
        if self.enabled() {
            return Ok(());
        }
        let mut settings = self.settings().await;
        let addresses = tokio::task::spawn_blocking(super::discovery::addresses)
            .await
            .map_err(|_| Error::execution())?;
        let private =
            |entry: &crate::contracts::ConnectionAddress| match entry.address.parse::<IpAddr>() {
                Ok(IpAddr::V4(ip)) => ip.is_private(),
                Ok(IpAddr::V6(ip)) => ip.is_unique_local(),
                Err(_) => false,
            };
        let local_addresses: Vec<_> = addresses.iter().filter(|entry| private(entry)).collect();
        if !local_addresses
            .iter()
            .any(|a| a.address == settings.address)
        {
            settings.address = local_addresses
                .first()
                .map(|a| a.address.clone())
                .ok_or_else(|| {
                    Error::new(
                        ErrorCode::ExecutionFailed,
                        "No usable local network. Connect your computer to Wi-Fi or Ethernet.",
                    )
                })?;
        }
        settings.enabled = true;
        self.configure(app.clone(), settings, true).await?;
        self.automatic.store(true, Ordering::Release);
        let mut monitor = self.monitor.lock().await;
        if monitor.is_none() {
            let Some(network) = app.network.clone() else {
                return Ok(());
            };
            *monitor = Some(tokio::spawn(async move {
                let mut tick = tokio::time::interval(std::time::Duration::from_secs(5));
                loop {
                    tick.tick().await;
                    if !network.enabled() || !network.automatic.load(Ordering::Acquire) {
                        continue;
                    }
                    let Ok(addresses) =
                        tokio::task::spawn_blocking(super::discovery::addresses).await
                    else {
                        continue;
                    };
                    let mut settings = network.settings().await;
                    if addresses
                        .iter()
                        .any(|entry| entry.address == settings.address)
                    {
                        continue;
                    }
                    if let Some(entry) =
                        addresses
                            .iter()
                            .find(|entry| match entry.address.parse::<IpAddr>() {
                                Ok(IpAddr::V4(ip)) => ip.is_private(),
                                Ok(IpAddr::V6(ip)) => ip.is_unique_local(),
                                Err(_) => false,
                            })
                    {
                        settings.address = entry.address.clone();
                        // A failed change leaves the last listener and persisted settings intact.
                        let _ = network.configure(app.clone(), settings, true).await;
                    }
                }
            }));
        }
        Ok(())
    }
    pub async fn apply(&self, app: App, settings: NetworkSettings) -> Result<()> {
        self.configure(app, settings, false).await?;
        self.automatic.store(false, Ordering::Release);
        Ok(())
    }
    async fn configure(
        &self,
        app: App,
        mut settings: NetworkSettings,
        automatic: bool,
    ) -> Result<()> {
        crate::domain::validate(
            "NetworkSettings",
            &serde_json::to_value(&settings).map_err(|_| Error::invalid())?,
        )?;
        let address = settings.address.parse::<IpAddr>().ok();
        if settings.enabled
            && !address
                .is_some_and(|a| !a.is_loopback() && !a.is_unspecified() && !a.is_multicast())
        {
            return Err(Error::new(
                ErrorCode::InvalidInput,
                "Choose this computer's Wi-Fi or Ethernet IP address",
            ));
        }
        let mut state = self.state.lock().await;
        // Two concurrent wizard starts reuse the listener even after fallback changed its port.
        if automatic && self.enabled() && settings.enabled && state.0.address == settings.address {
            return Ok(());
        }
        let unchanged = self.enabled()
            && settings.enabled
            && state.0.address == settings.address
            && state.0.port == settings.port;
        let listener = if settings.enabled
            && !unchanged
            && !(self.external && settings.port == u64::from(self.primary_port))
        {
            let address = address.expect("validated address");
            let bound =
                tokio::net::TcpListener::bind(SocketAddr::new(address, settings.port as u16)).await;
            let listener = match bound {
                Ok(listener) => listener,
                Err(error) if automatic && error.kind() == std::io::ErrorKind::AddrInUse => {
                    let listener = tokio::net::TcpListener::bind(SocketAddr::new(address, 0))
                        .await
                        .map_err(|_| Error::execution())?;
                    settings.port = listener
                        .local_addr()
                        .map_err(|_| Error::execution())?
                        .port()
                        .into();
                    listener
                }
                Err(_) => return Err(Error::new(
                    ErrorCode::ExecutionFailed,
                    "Cannot enable phone access: address is not on this computer or port is busy",
                )),
            };
            Some(listener)
        } else {
            None
        };
        let bytes = serde_json::to_vec_pretty(&settings).map_err(|_| Error::invalid())?;
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || crate::storage::atomic_replace(&path, &bytes))
            .await
            .map_err(|_| Error::execution())??;
        if !unchanged {
            self.enabled.store(false, Ordering::Release);
            if let Some((mut task, stop)) = state.1.take() {
                let _ = stop.send(());
                if tokio::time::timeout(std::time::Duration::from_secs(2), &mut task)
                    .await
                    .is_err()
                {
                    task.abort();
                }
            }
            if let Some(listener) = listener {
                let mut remote_app = app;
                remote_app.port = settings.port as u16;
                let routes = router(remote_app);
                let (stop, cancelled) = tokio::sync::oneshot::channel();
                state.1 = Some((
                    tokio::spawn(async move {
                        if let Err(error) = axum::serve(
                            listener,
                            routes.into_make_service_with_connect_info::<SocketAddr>(),
                        )
                        .with_graceful_shutdown(async {
                            let _ = cancelled.await;
                        })
                        .await
                        {
                            eprintln!("Phone listener: {error}");
                        }
                    }),
                    stop,
                ));
            }
        }
        self.enabled.store(settings.enabled, Ordering::Release);
        state.0 = settings;
        Ok(())
    }
    pub async fn stop(&self) {
        if let Some(monitor) = self.monitor.lock().await.take() {
            monitor.abort();
        }
        self.automatic.store(false, Ordering::Release);
        self.enabled.store(false, Ordering::Release);
        if let Some((mut task, stop)) = self.state.lock().await.1.take() {
            let _ = stop.send(());
            if tokio::time::timeout(std::time::Duration::from_secs(2), &mut task)
                .await
                .is_err()
            {
                task.abort();
            }
        }
    }
}
