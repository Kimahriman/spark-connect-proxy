use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{Path, State},
    routing::get,
    Extension, Json, Router,
};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use tower::ServiceBuilder;
use tower_http::auth::AsyncRequireAuthorizationLayer;
use uuid::Uuid;

use crate::{
    auth::{TestAuth, UserId},
    store::{InMemorySessionStore, Session, SessionStore},
};

pub fn get_router() -> Router {
    let session_store = InMemorySessionStore::default();
    Router::new()
        .route("/sessions", get(list_sessions).post(create_session))
        .route(
            "/sessions/:session_id",
            get(get_session).delete(delete_session),
        )
        .layer(ServiceBuilder::new().layer(AsyncRequireAuthorizationLayer::new(TestAuth {})))
        .with_state(AppStateDyn {
            session_store: Arc::new(session_store),
        })
}

#[derive(Clone)]
struct AppStateDyn {
    session_store: Arc<dyn SessionStore>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct CreateSessionRequest {
    config: HashMap<String, String>,
}

#[derive(Serialize)]
struct CreateSessionResponse {
    token: String,
}

async fn create_session(
    State(state): State<AppStateDyn>,
    Extension(user): Extension<UserId>,
    Json(_params): Json<CreateSessionRequest>,
) -> Json<CreateSessionResponse> {
    let token = Uuid::new_v4().to_string();
    state.session_store.create_session(&user.0, token.clone());
    Json(CreateSessionResponse { token })
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
