use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Extension, Json, Router,
};
use http::StatusCode;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use tower::ServiceBuilder;
use tower_http::auth::AsyncRequireAuthorizationLayer;
use uuid::Uuid;

use crate::{
    auth::{BearerToken, TokenAuth, UserAuth, UserId},
    config::ProxyConfig,
    launcher::Launcher,
    store::{Session, SessionStore},
};

pub fn get_router(config: &ProxyConfig, session_store: Arc<dyn SessionStore>) -> Router {
    let app_state = AppStateDyn {
        session_store,
        launcher: Arc::new(Launcher::from_config(config)),
    };

    let user_api = Router::new()
        .route("/sessions", get(list_sessions).post(create_session))
        .route(
            "/sessions/:session_id",
            get(get_session).delete(delete_session),
        )
        .route("/versions", get(list_versions))
        .route_layer(ServiceBuilder::new().layer(AsyncRequireAuthorizationLayer::new(UserAuth {})))
        .with_state(app_state.clone());

    let callback_api = Router::new()
        .route("/callback", post(session_callback))
        .route_layer(ServiceBuilder::new().layer(AsyncRequireAuthorizationLayer::new(TokenAuth {})))
        .with_state(app_state);

    Router::new().merge(user_api).merge(callback_api)
}

#[derive(Clone)]
struct AppStateDyn {
    session_store: Arc<dyn SessionStore>,
    launcher: Arc<Launcher>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct CreateSessionRequest {
    version: Option<String>,
    config: Option<HashMap<String, String>>,
}

#[derive(Serialize)]
struct CreateSessionResponse {
    token: String,
}

async fn create_session(
    State(state): State<AppStateDyn>,
    Extension(user): Extension<UserId>,
    Json(params): Json<CreateSessionRequest>,
) -> Result<Json<CreateSessionResponse>, StatusCode> {
    let token = Uuid::new_v4().to_string();
    state.session_store.create_session(&user.0, token.clone());

    state
        .launcher
        .launch(
            params.version.as_ref().map(|v| v.as_ref()),
            user.0,
            token.clone(),
            params.config.unwrap_or_default(),
        )
        .await
        .map_err(|e| {
            warn!("{:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(CreateSessionResponse { token }))
}

async fn get_session(
    State(state): State<AppStateDyn>,
    Path(session_id): Path<u64>,
    Extension(user): Extension<UserId>,
) -> Result<Json<Session>, StatusCode> {
    let session = state
        .session_store
        .get_session(&user.0, session_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(session))
}

async fn list_sessions(
    State(state): State<AppStateDyn>,
    Extension(user): Extension<UserId>,
) -> Json<Vec<Session>> {
    Json(state.session_store.list_sessions(&user.0))
}

async fn delete_session(
    State(state): State<AppStateDyn>,
    Path(session_id): Path<u64>,
    Extension(user): Extension<UserId>,
) {
    state.session_store.delete_session(&user.0, session_id);
}

async fn list_versions(State(state): State<AppStateDyn>) -> Json<Vec<String>> {
    Json(state.launcher.get_versions())
}

#[derive(Deserialize)]
struct SessionCallbackRequest {
    address: String,
}

async fn session_callback(
    State(state): State<AppStateDyn>,
    Extension(token): Extension<BearerToken>,
    Json(params): Json<SessionCallbackRequest>,
) -> Result<(), StatusCode> {
    info!("Got the callback for {}", token.0);
    state
        .session_store
        .set_session_addr(token.0.as_ref(), params.address);
    Ok(())
}
