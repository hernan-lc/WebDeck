use super::*;
use socketioxide::{
    extract::{Data, SocketRef},
    layer::SocketIoLayer,
    SocketIo,
};
pub(super) fn layer(app: App) -> SocketIoLayer {
    let (layer, io) = SocketIo::new_layer();
    io.ns("/v2", move |socket: SocketRef, Data(auth): Data<Value>| {
        on_connect(app.clone(), socket, auth)
    });
    layer
}
async fn on_connect(app: App, socket: SocketRef, auth: Value) {
    let peer = socket
        .req_parts()
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|p| p.0.ip());
    let Some(peer) = peer else {
        let _ = socket.disconnect();
        return;
    };
    if !auth.is_object() {
        let _ = socket.disconnect();
        return;
    }
    let token = match auth.get("token") {
        None => super::token(&socket.req_parts().headers).unwrap_or_else(|_| Some(String::new())),
        Some(Value::String(t)) => Some(t.clone()),
        Some(_) => Some(String::new()),
    };
    let identity = match authorize(&app, peer, token.clone()).await {
        Ok(identity) => identity,
        Err(_) => {
            let _ = socket.disconnect();
            return;
        }
    };
    let device_id = if let Some(credential) = token.clone() {
        let sessions = app.sessions.clone();
        match blocking(app.authorization.clone(), move || {
            sessions.connected(&credential)
        })
        .await
        {
            Ok(id) => Some(id),
            Err(_) => {
                let _ = socket.disconnect();
                return;
            }
        }
    } else {
        None
    };
    let stopping = Arc::new(tokio::sync::Notify::new());
    let stop = stopping.clone();
    let sessions = app.sessions.clone();
    let disconnected_id = device_id.clone();
    socket.on_disconnect(async move || {
        if let Some(id) = disconnected_id {
            sessions.disconnected(&id);
            tokio::task::spawn_blocking(move || {
                if sessions.persist_history().is_err() {
                    eprintln!("Cannot save device connection history");
                }
            });
        }
        stop.notify_waiters();
    });
    let mut changes = app.sessions.subscribe();
    let observer = socket.clone();
    let owner = app.clone();
    let credential = token.clone();
    let cancelled = stopping.clone();
    let local = identity.local;
    tokio::spawn(async move {
        let mut check = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            tokio::select! {
                _ = cancelled.notified() => break,
                _ = check.tick() => {
                    if !observer.connected() { break; }
                    if let Err(error) = authorize(&owner, peer, credential.clone()).await {
                        if matches!(error.code, ErrorCode::CapacityExhausted | ErrorCode::PersistenceFailed) { continue; }
                        let event = if error.code == ErrorCode::DeviceRevoked { "device.revoked" } else { "device.authorization_expired" };
                        let _ = observer.emit(event, &json!({"api_version":2}));
                        let _ = observer.disconnect(); break;
                    }
                }
                received = changes.recv() => {
                    match received {
                        Ok(event) => {
                            let own = event.device_id.is_some() && event.device_id == device_id;
                            if own && event.name == "device.revoked" { let _ = observer.emit(event.name, &event.value); let _ = observer.disconnect(); break; }
                            if (local || own) && authorize(&owner, peer, credential.clone()).await.is_ok() { let _ = observer.emit(event.name, &event.value); }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => { let _ = observer.emit("connection.status", &json!({"resync":true})); }
                        Err(_) => break,
                    }
                }
            }
        }
    });
    let _ = socket.emit(
        "connection.status",
        &json!({"api_version":2,"authenticated":true,"realtime_connected":true}),
    );
    if let Some(mut events) = app.executor.events() {
        let observer = socket.clone();
        let owner = app.clone();
        let credential = token.clone();
        tokio::spawn(async move {
            loop {
                let received = tokio::select! { _ = stopping.notified() => break, result = events.recv() => result };
                match received {
                    Ok(event) => {
                        if !observer.connected() {
                            break;
                        }
                        if authorize(&owner, peer, credential.clone())
                            .await
                            .is_ok_and(|i| i.capabilities.contains(&Capability::Read))
                        {
                            let _ = observer.emit("runtime_event", &event);
                        } else {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
        });
    }
    let commands = app.clone();
    let credential = token.clone();
    socket.on("command", move |s: SocketRef, Data(v): Data<Value>| {
        invoke(commands.clone(), s, credential.clone(), peer, v)
    });
    socket.on("usage", move |s: SocketRef, Data(v): Data<Value>| {
        usage(app.clone(), s, token.clone(), peer, v)
    });
}
async fn invoke(app: App, socket: SocketRef, token: Option<String>, peer: IpAddr, v: Value) {
    let id = v["request_id"].as_str().unwrap_or("").to_string();
    let result = async {
        domain::validate("CommandRequest", &v)?;
        let r: CommandRequest = serde_json::from_value(v).map_err(|_| Error::invalid())?;
        let caps = authorize(&app, peer, token).await?.capabilities;
        let r = resolve_async(&app, r).await?;
        let observer = socket.clone();
        let correlation = id.clone();
        app.executor
            .execute(r, caps, move || {
                let _ = observer.emit(
                    "command_result",
                    &json!({"api_version":2,"request_id":correlation,"state":"accepted"}),
                );
            })
            .await
    }
    .await;
    let _ = socket.emit("command_result", &command_event(id, result));
}
async fn usage(app: App, socket: SocketRef, token: Option<String>, peer: IpAddr, v: Value) {
    let id = v["request_id"].as_str().unwrap_or("");
    if id.is_empty() || id.len() > 128 {
        return;
    }
    if authorize(&app, peer, token)
        .await
        .is_ok_and(|c| c.capabilities.contains(&Capability::Read))
    {
        if let Ok(value) = blocking(app.queries.clone(), || Ok(crate::capabilities::usage())).await
        {
            let _ = socket.emit(
                "usage_result",
                &json!({"api_version":2,"request_id":id,"usage":value}),
            );
        }
    } else {
        let _ = socket.disconnect();
    }
}
