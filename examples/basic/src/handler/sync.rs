//! The routes that operate the replica's synchronisation task.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use turso_orm::ReplicaSync;

use crate::dto::sync::{ReplicaStatus, SyncReport};
use crate::error::ApiError;
use crate::service::sync;
use crate::state::AppState;

// --8<-- [start:sync]
/// The synchronisation task, which only the `sync` mode starts; the other
/// modes answer `409 Conflict`, since they have nothing to synchronise.
fn task(state: &AppState) -> Result<&ReplicaSync, ApiError> {
    state.sync().ok_or(ApiError::NotAReplica)
}

/// `POST /sync`: runs a round now, serialised with the task's own rounds.
pub async fn run(State(state): State<AppState>) -> Result<Json<SyncReport>, ApiError> {
    let status = sync::round(task(&state)?).await?;
    Ok(Json(status.into()))
}

/// `GET /sync/stats`: what the last round saw, without touching the engine.
pub async fn stats(State(state): State<AppState>) -> Result<Json<ReplicaStatus>, ApiError> {
    Ok(Json(task(&state)?.status().into()))
}
// --8<-- [end:sync]

/// `POST /sync/checkpoint`.
pub async fn checkpoint(State(state): State<AppState>) -> Result<StatusCode, ApiError> {
    task(&state)?;
    sync::checkpoint(state.db()).await?;
    Ok(StatusCode::NO_CONTENT)
}
