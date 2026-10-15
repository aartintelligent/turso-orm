//! The error of the API: how a `DbErr` becomes an HTTP status.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use turso_orm::DbErr;

/// What a handler can fail with.
#[derive(Debug)]
pub enum ApiError {
    /// The named resource does not exist.
    NotFound(&'static str),
    /// The database refused or failed the operation.
    Db(DbErr),
}

impl From<DbErr> for ApiError {
    fn from(err: DbErr) -> Self {
        Self::Db(err)
    }
}

// --8<-- [start:status]
/// Maps each error to a status in one place, so that handlers only use `?`.
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::NotFound(what) => (StatusCode::NOT_FOUND, format!("{what} not found")),
            Self::Db(err) if err.is_unique_violation() => (StatusCode::CONFLICT, err.to_string()),
            Self::Db(err) if err.is_foreign_key_violation() => {
                (StatusCode::UNPROCESSABLE_ENTITY, err.to_string())
            }
            Self::Db(err) if err.is_busy() => (StatusCode::SERVICE_UNAVAILABLE, err.to_string()),
            Self::Db(DbErr::RecordNotFound(what)) => {
                (StatusCode::NOT_FOUND, format!("{what} not found"))
            }
            Self::Db(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}
// --8<-- [end:status]
