//! The route that synchronises an embedded replica.

use axum::Json;
use axum::extract::State;

use crate::dto::sync::SyncReport;
use crate::error::ApiError;
use crate::service::sync;
use crate::settings::Mode;
use crate::state::AppState;

// --8<-- [start:sync]
/// `POST /sync`: pushes the replica's writes and pulls the cloud's.
///
/// Only the `sync` mode has a replica; the other modes answer `409 Conflict`
/// before anything is attempted.
pub async fn run(State(state): State<AppState>) -> Result<Json<SyncReport>, ApiError> {
    if state.settings.mode != Mode::Sync {
        return Err(ApiError::NotAReplica);
    }
    let pulled = sync::push_pull(&state.db).await?;
    Ok(Json(SyncReport { pulled }))
}
// --8<-- [end:sync]
