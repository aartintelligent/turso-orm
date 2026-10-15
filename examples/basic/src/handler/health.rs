//! The health probe.

use axum::extract::State;
use axum::http::StatusCode;

use crate::error::ApiError;
use crate::state::AppState;

/// `GET /health`: `204 No Content` when the database answers, the database
/// error's status otherwise.
pub async fn check(State(state): State<AppState>) -> Result<StatusCode, ApiError> {
    state.readiness().await?;
    Ok(StatusCode::NO_CONTENT)
}
