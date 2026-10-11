use super::*;

pub(super) async fn devices(
    State(a): State<App>,
    axum::Extension(i): axum::Extension<Identity>,
) -> Result<Json<DeviceList>> {
    i.local()?;
    i.require(Capability::Settings)?;
    Ok(Json(DeviceList {
        api_version: 2,
        devices: blocking(a.io.clone(), move || Ok(a.sessions.list())).await?,
    }))
}
pub(super) async fn approve(
    State(a): State<App>,
    axum::Extension(i): axum::Extension<Identity>,
    Json(r): Json<DeviceRequest>,
) -> Result<Json<DeviceApproval>> {
    i.local()?;
    i.require(Capability::Settings)?;
    Ok(Json(
        blocking(a.io.clone(), move || a.sessions.approve(r)).await?,
    ))
}
pub(super) async fn revoke(
    State(a): State<App>,
    axum::Extension(i): axum::Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    i.local()?;
    i.require(Capability::Settings)?;
    blocking(a.io.clone(), move || a.sessions.revoke(&id)).await?;
    Ok(Json(json!({"api_version":2,"revoked":true})))
}

pub(super) async fn update(
    State(a): State<App>,
    axum::Extension(i): axum::Extension<Identity>,
    Path(id): Path<String>,
    Json(value): Json<Value>,
) -> Result<Json<Device>> {
    i.local()?;
    i.require(Capability::Settings)?;
    domain::validate("DeviceUpdate", &value)?;
    let update = serde_json::from_value(value).map_err(|_| Error::invalid())?;
    Ok(Json(
        blocking(a.io.clone(), move || a.sessions.update(&id, update)).await?,
    ))
}
pub(super) async fn presence(
    State(a): State<App>,
    axum::Extension(i): axum::Extension<Identity>,
) -> Result<Json<DevicePresenceList>> {
    i.local()?;
    i.require(Capability::Settings)?;
    Ok(Json(
        blocking(a.authorization.clone(), move || {
            Ok(a.sessions.device_presence())
        })
        .await?,
    ))
}
pub(super) async fn device_self(State(a): State<App>, headers: HeaderMap) -> Result<Json<Device>> {
    let token = super::token(&headers)?
        .ok_or_else(|| Error::new(ErrorCode::Unauthorized, "Device authorization required"))?;
    Ok(Json(
        blocking(a.authorization.clone(), move || a.sessions.identify(&token)).await?,
    ))
}

pub(super) async fn renew(
    State(a): State<App>,
    axum::Extension(i): axum::Extension<Identity>,
    Path(id): Path<String>,
    Json(value): Json<Value>,
) -> Result<Json<Device>> {
    i.local()?;
    i.require(Capability::Settings)?;
    domain::validate("DeviceRenew", &value)?;
    let request: DeviceRenew = serde_json::from_value(value).map_err(|_| Error::invalid())?;
    Ok(Json(
        blocking(a.io.clone(), move || {
            a.sessions.renew(&id, request.ttl_seconds)
        })
        .await?,
    ))
}
