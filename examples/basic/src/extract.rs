//! The extractors of the API: axum's, with their rejections turned into
//! `ApiError`.
//!
//! Left alone, a malformed body or an unparsable path segment is answered
//! by axum as `text/plain`, outside the `{"error": …}` envelope every other
//! failure uses. These wrappers keep the envelope: a client reads one shape
//! whatever went wrong.

use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts, Json, Path, Query, Request};
use axum::http::request::Parts;

use crate::error::ApiError;

// --8<-- [start:extractors]
/// A JSON body, or `400 Bad Request` in the envelope when it does not parse.
pub struct ApiJson<T>(pub T);

impl<S, T> FromRequest<S> for ApiJson<T>
where
    S: Send + Sync,
    Json<T>: FromRequest<S, Rejection = JsonRejection>,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(|rejection| ApiError::BadRequest(rejection.body_text()))?;
        Ok(Self(value))
    }
}

/// Path segments, or `400 Bad Request` in the envelope when they do not parse.
pub struct ApiPath<T>(pub T);

impl<S, T> FromRequestParts<S> for ApiPath<T>
where
    S: Send + Sync,
    Path<T>: FromRequestParts<S, Rejection = PathRejection>,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(value) = Path::<T>::from_request_parts(parts, state)
            .await
            .map_err(|rejection| ApiError::BadRequest(rejection.body_text()))?;
        Ok(Self(value))
    }
}

/// Query parameters, or `400 Bad Request` in the envelope when they do not parse.
pub struct ApiQuery<T>(pub T);

impl<S, T> FromRequestParts<S> for ApiQuery<T>
where
    S: Send + Sync,
    Query<T>: FromRequestParts<S, Rejection = QueryRejection>,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(|rejection| ApiError::BadRequest(rejection.body_text()))?;
        Ok(Self(value))
    }
}
// --8<-- [end:extractors]
