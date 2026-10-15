//! Routes of the tools a thread invoked, in both directions.

use crate::entity::{thread, tool};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use turso_orm::Database;

use crate::dto::thread::ThreadWithTools;
use crate::dto::thread_tool::UseTool;
use crate::error::ApiError;
use crate::extract::{ApiJson, ApiPath};
use crate::handler::thread::load;
use crate::service::thread_tool as service;

/// `POST /threads/{id}/tools`.
pub async fn use_tool(
    State(db): State<Database>,
    ApiPath(id): ApiPath<i32>,
    ApiJson(body): ApiJson<UseTool>,
) -> Result<(StatusCode, Json<tool::Model>), ApiError> {
    load(&db, id).await?;
    let tool = service::record_tool_use(&db, id, &body.name).await?;
    Ok((StatusCode::CREATED, Json(tool)))
}

/// `GET /threads/{id}/tools`.
pub async fn tools(
    State(db): State<Database>,
    ApiPath(id): ApiPath<i32>,
) -> Result<Json<Vec<tool::Model>>, ApiError> {
    let thread = load(&db, id).await?;
    Ok(Json(service::tools_of_thread(&db, &thread).await?))
}

/// `GET /threads/using/{tool}`.
pub async fn using(
    State(db): State<Database>,
    ApiPath(tool): ApiPath<String>,
) -> Result<Json<Vec<thread::Model>>, ApiError> {
    Ok(Json(service::threads_using(&db, &tool).await?))
}

/// `GET /threads/with-tools`.
pub async fn with_tools(
    State(db): State<Database>,
) -> Result<Json<Vec<ThreadWithTools>>, ApiError> {
    Ok(Json(service::threads_with_tools(&db).await?))
}
