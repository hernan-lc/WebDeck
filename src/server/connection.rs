use super::*;
fn manager(a: &App, i: &Identity) -> Result<Arc<crate::network::Network>> {
    i.local()?;
    i.require(Capability::Settings)?;
    a.network.clone().ok_or_else(|| {
        Error::new(
            ErrorCode::ExecutionFailed,
            "Connection management unavailable",
        )
    })
}
pub(super) async fn status(
    State(a): State<App>,
    axum::Extension(i): axum::Extension<Identity>,
) -> Result<Json<ConnectionStatus>> {
    let network = manager(&a, &i)?;
    let sessions = a.sessions.clone();
    let (addresses, reachable) = connection_query(a.queries.clone(), move || {
        Ok((
            crate::connection::discovery::addresses(),
            sessions
                .device_presence()
                .devices
                .iter()
                .any(|device| device.online)
                || !sessions.pending_pairings().requests.is_empty(),
        ))
    })
    .await?;
    Ok(Json(ConnectionStatus {
        api_version: 2,
        listener_started: network.enabled(),
        network_detected: !addresses.is_empty(),
        client_reachability_verified: reachable,
        network: network.status().await,
        addresses,
    }))
}
pub(super) async fn start(
    State(a): State<App>,
    axum::Extension(i): axum::Extension<Identity>,
) -> Result<Json<ConnectionStatus>> {
    manager(&a, &i)?.start(a.clone()).await?;
    status(State(a), axum::Extension(i)).await
}
pub(super) async fn invitation(
    State(a): State<App>,
    axum::Extension(i): axum::Extension<Identity>,
) -> Result<Json<ConnectionInvitation>> {
    let network = manager(&a, &i)?;
    let status = network.status().await;
    if !status.settings.enabled {
        return Err(Error::new(
            ErrorCode::InvalidInput,
            "Start device connections first",
        ));
    }
    let (id, expires_at) =
        blocking(a.authorization.clone(), move || a.sessions.invitation()).await?;
    Ok(Json(ConnectionInvitation {
        api_version: 2,
        url: format!("{}#connect={id}", status.url),
        id,
        expires_at,
    }))
}
pub(super) async fn qr(
    State(a): State<App>,
    axum::Extension(i): axum::Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Response> {
    let network = manager(&a, &i)?;
    let sessions = a.sessions.clone();
    let candidate = id.clone();
    blocking(a.authorization.clone(), move || {
        sessions.validate_invitation(&candidate)
    })
    .await?;
    let status = network.status().await;
    if !status.settings.enabled {
        return Err(Error::invalid());
    }
    let url = format!("{}#connect={id}", status.url);
    let png = connection_query(a.queries.clone(), move || {
        crate::qr::generate_qr_code_png(&url).ok_or_else(Error::execution)
    })
    .await?;
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "image/png"),
            (axum::http::header::CACHE_CONTROL, "no-store"),
        ],
        png,
    )
        .into_response())
}
pub(super) async fn profiles(
    State(a): State<App>,
    axum::Extension(i): axum::Extension<Identity>,
) -> Result<Json<PermissionProfiles>> {
    i.local()?;
    i.require(Capability::Settings)?;
    let recommended = connection_query(a.queries.clone(), move || {
        crate::connection::permissions::recommended(&a)
    })
    .await?;
    Ok(Json(PermissionProfiles {
        api_version: 2,
        recommended,
        full: crate::connection::permissions::REMOTE.to_vec(),
    }))
}
pub(super) async fn permission_request(
    State(a): State<App>,
    headers: HeaderMap,
    Json(value): Json<Value>,
) -> Result<Json<PermissionPending>> {
    domain::validate("PermissionRequest", &value)?;
    let request: PermissionRequest = serde_json::from_value(value).map_err(|_| Error::invalid())?;
    let token = super::token(&headers)?
        .ok_or_else(|| Error::new(ErrorCode::Unauthorized, "Device authorization required"))?;
    Ok(Json(
        blocking(a.authorization.clone(), move || {
            a.sessions.request_permissions(&token, request.capabilities)
        })
        .await?,
    ))
}
pub(super) async fn permissions(
    State(a): State<App>,
    axum::Extension(i): axum::Extension<Identity>,
) -> Result<Json<PermissionRequests>> {
    i.local()?;
    i.require(Capability::Settings)?;
    Ok(Json(
        blocking(a.authorization.clone(), move || {
            Ok(a.sessions.permission_requests())
        })
        .await?,
    ))
}
pub(super) async fn approve_permission(
    State(a): State<App>,
    axum::Extension(i): axum::Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<PermissionRequests>> {
    decide(a, i, id, true).await
}
pub(super) async fn reject_permission(
    State(a): State<App>,
    axum::Extension(i): axum::Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<PermissionRequests>> {
    decide(a, i, id, false).await
}
async fn decide(
    a: App,
    i: Identity,
    id: String,
    approve: bool,
) -> Result<Json<PermissionRequests>> {
    i.local()?;
    i.require(Capability::Settings)?;
    Ok(Json(
        blocking(a.io.clone(), move || {
            a.sessions.decide_permissions(&id, approve)?;
            Ok(a.sessions.permission_requests())
        })
        .await?,
    ))
}

pub(super) async fn progress(
    State(a): State<App>,
    axum::Extension(i): axum::Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<ConnectionProgress>> {
    manager(&a, &i)?;
    Ok(Json(
        blocking(a.authorization.clone(), move || {
            Ok(a.sessions.invitation_progress(&id))
        })
        .await?,
    ))
}
pub(super) async fn cancel(
    State(a): State<App>,
    axum::Extension(i): axum::Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<ConnectionProgress>> {
    manager(&a, &i)?;
    blocking(a.authorization.clone(), move || {
        a.sessions.cancel_invitation(&id)
    })
    .await?;
    Ok(Json(ConnectionProgress {
        api_version: 2,
        state: "rejected".into(),
    }))
}

pub(super) async fn permission_progress(
    State(a): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ConnectionProgress>> {
    let token = super::token(&headers)?
        .ok_or_else(|| Error::new(ErrorCode::Unauthorized, "Device authorization required"))?;
    Ok(Json(
        blocking(a.authorization.clone(), move || {
            a.sessions.permission_progress(&token, &id)
        })
        .await?,
    ))
}

// Host-only discovery and QR reads can overlap during the wizard. Wait briefly for
// the existing bounded pool instead of failing a normal first connection.
async fn connection_query<T: Send + 'static>(
    admission: Arc<tokio::sync::Semaphore>,
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    let permit = tokio::time::timeout(std::time::Duration::from_secs(2), admission.acquire_owned())
        .await
        .map_err(|_| {
            Error::new(
                ErrorCode::CapacityExhausted,
                "Connection setup is busy. Retry shortly.",
            )
        })?
        .map_err(|_| Error::execution())?;
    tokio::task::spawn_blocking(move || {
        let _owned = permit;
        work()
    })
    .await
    .map_err(|_| Error::execution())?
}
