//! The routes that operate an embedded replica.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use turso_orm::Database;

use crate::dto::sync::{ReplicaStatus, SyncReport};
use crate::error::ApiError;
use crate::service::sync;
use crate::settings::Mode;
use crate::state::AppState;

// --8<-- [start:sync]
/// The pool, when the database is an embedded replica; `409 Conflict`
/// otherwise, since the other modes have nothing to synchronise.
fn replica(state: &AppState) -> Result<&Database, ApiError> {
    if state.settings.mode == Mode::Sync {
        Ok(&state.db)
    } else {
        Err(ApiError::NotAReplica)
    }
}

/// `POST /sync`: pushes the replica's writes and pulls the cloud's.
pub async fn run(State(state): State<AppState>) -> Result<Json<SyncReport>, ApiError> {
    let pulled = sync::push_pull(replica(&state)?).await?;
    Ok(Json(SyncReport { pulled }))
}
// --8<-- [end:sync]

/// `POST /sync/checkpoint`.
pub async fn checkpoint(State(state): State<AppState>) -> Result<StatusCode, ApiError> {
    sync::checkpoint(replica(&state)?).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /sync/stats`.
pub async fn stats(State(state): State<AppState>) -> Result<Json<ReplicaStatus>, ApiError> {
    Ok(Json(sync::stats(replica(&state)?).await?.into()))
}
