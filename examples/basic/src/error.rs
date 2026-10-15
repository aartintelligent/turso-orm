//! The error of the API: how a `DbErr` and a refused request become an HTTP
//! status, in one place, so that handlers only use `?`.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use turso_orm::DbErr;

/// What a handler can fail with.
#[derive(Debug)]
pub enum ApiError {
    /// The request could not be read: a body or a parameter that does not
    /// parse. The text is axum's.
    BadRequest(String),
    /// The named resource does not exist.
    NotFound(&'static str),
    /// The database is not an embedded replica, so there is nothing to
    /// synchronise.
    NotAReplica,
    /// The database refused or failed the operation.
    Db(DbErr),
}

impl From<DbErr> for ApiError {
    fn from(err: DbErr) -> Self {
        Self::Db(err)
    }
}

// --8<-- [start:status]
/// Maps each error to a status and logs it where it is answered: a refused
/// request at `warn` with its message, a failure at `error` with its cause,
/// which the client does not see.
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::NotFound(what) => (StatusCode::NOT_FOUND, format!("{what} not found")),
            Self::NotAReplica => (
                StatusCode::CONFLICT,
                "the database is not an embedded replica".to_owned(),
            ),
            Self::Db(err) => database(err),
        };
        if status.is_server_error() {
            tracing::error!(%status, "request failed");
        } else {
            tracing::warn!(%status, %message, "request refused");
        }
        (status, Json(json!({ "error": message }))).into_response()
    }
}

/// The status of a database error: the constraint kinds and the busy
/// condition the driver classifies, the missing row, and the rest as a
/// failure whose detail stays in the log.
fn database(err: DbErr) -> (StatusCode, String) {
    if err.is_unique_violation() {
        (StatusCode::CONFLICT, err.to_string())
    } else if err.is_foreign_key_violation() {
        (StatusCode::UNPROCESSABLE_ENTITY, err.to_string())
    } else if err.is_busy() {
        (StatusCode::SERVICE_UNAVAILABLE, err.to_string())
    } else if let DbErr::RecordNotFound(what) = &err {
        (StatusCode::NOT_FOUND, format!("{what} not found"))
    } else {
        tracing::error!(%err, "database error");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal error".to_owned(),
        )
    }
}
// --8<-- [end:status]
