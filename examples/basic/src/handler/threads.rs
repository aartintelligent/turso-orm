//! Routes of the threads, their records and their tools.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use entity::prelude::Thread;
use entity::{thread, thread_record, tool};
use turso_orm::prelude::*;

use crate::dto::page::After;
use crate::dto::thread::{
    AppendRecord, Fork, OpenThread, OpenWithTools, Purge, ThreadSummary, ThreadWithRecords,
    ThreadWithTools, UseTool,
};
use crate::error::ApiError;
use crate::service::threads;

/// The thread named by the path, or `404 Not Found`.
async fn load(db: &Database, id: i32) -> Result<thread::Model, ApiError> {
    Thread::find_by_id(id)
        .one(db)
        .await?
        .ok_or(ApiError::NotFound("thread"))
}

/// `POST /threads`.
pub async fn open(
    State(db): State<Database>,
    Json(body): Json<OpenThread>,
) -> Result<(StatusCode, Json<thread::Model>), ApiError> {
    let opened = threads::open_thread(&db, body).await?;
    Ok((StatusCode::CREATED, Json(opened)))
}

/// `POST /threads/json`: a raw object, every key naming a column is set.
pub async fn open_from_json(
    State(db): State<Database>,
    Json(body): Json<serde_json::Value>,
) -> Result<(StatusCode, Json<thread::Model>), ApiError> {
    let opened = threads::open_thread_from_json(&db, body).await?;
    Ok((StatusCode::CREATED, Json(opened)))
}

/// `POST /threads/with-tools`.
pub async fn open_with_tools(
    State(db): State<Database>,
    Json(body): Json<OpenWithTools>,
) -> Result<(StatusCode, Json<ThreadWithTools>), ApiError> {
    let opened = threads::open_thread_with_tools(&db, body).await?;
    Ok((StatusCode::CREATED, Json(opened)))
}

/// `POST /threads/{id}/complete`.
pub async fn complete(
    State(db): State<Database>,
    Path(id): Path<i32>,
) -> Result<Json<thread::Model>, ApiError> {
    threads::complete_thread(&db, id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("thread"))
}

/// `DELETE /threads?prefix=`.
pub async fn purge(
    State(db): State<Database>,
    Query(purge): Query<Purge>,
) -> Result<Json<Vec<thread::Model>>, ApiError> {
    Ok(Json(threads::purge_threads(&db, &purge.prefix).await?))
}

/// `GET /threads?after=&size=`.
pub async fn list(
    State(db): State<Database>,
    Query(after): Query<After>,
) -> Result<Json<Vec<thread::Model>>, ApiError> {
    Ok(Json(
        threads::threads_after(&db, after.after, after.size).await?,
    ))
}

/// `GET /threads/{id}`.
pub async fn get(
    State(db): State<Database>,
    Path(id): Path<i32>,
) -> Result<Json<ThreadWithRecords>, ApiError> {
    threads::thread_with_records(&db, id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("thread"))
}

/// `POST /threads/{id}/records`.
pub async fn append_records(
    State(db): State<Database>,
    Path(id): Path<i32>,
    Json(bodies): Json<Vec<AppendRecord>>,
) -> Result<(StatusCode, Json<Vec<thread_record::Model>>), ApiError> {
    load(&db, id).await?;
    let records = threads::append_records(&db, id, bodies).await?;
    Ok((StatusCode::CREATED, Json(records)))
}

/// `POST /threads/{id}/fork`.
pub async fn fork(
    State(db): State<Database>,
    Path(id): Path<i32>,
    Json(body): Json<Fork>,
) -> Result<(StatusCode, Json<thread::Model>), ApiError> {
    let parent = load(&db, id).await?;
    let forked = threads::fork_thread(&db, &parent, body.agent_id).await?;
    Ok((StatusCode::CREATED, Json(forked)))
}

/// `GET /threads/{id}/parent`.
pub async fn parent(
    State(db): State<Database>,
    Path(id): Path<i32>,
) -> Result<Json<thread::Model>, ApiError> {
    let fork = load(&db, id).await?;
    threads::parent_of(&db, &fork)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("parent"))
}

/// `GET /threads/{id}/forks`.
pub async fn forks(
    State(db): State<Database>,
    Path(id): Path<i32>,
) -> Result<Json<Vec<thread::Model>>, ApiError> {
    let root = load(&db, id).await?;
    Ok(Json(threads::forks_of(&db, &root).await?))
}

/// `GET /threads/reviewed-by/{name}`.
pub async fn reviewed_by(
    State(db): State<Database>,
    Path(name): Path<String>,
) -> Result<Json<Vec<thread::Model>>, ApiError> {
    Ok(Json(threads::reviewed_by(&db, &name).await?))
}

/// `POST /threads/{id}/tools`.
pub async fn use_tool(
    State(db): State<Database>,
    Path(id): Path<i32>,
    Json(body): Json<UseTool>,
) -> Result<(StatusCode, Json<tool::Model>), ApiError> {
    load(&db, id).await?;
    let tool = threads::record_tool_use(&db, id, &body.name).await?;
    Ok((StatusCode::CREATED, Json(tool)))
}

/// `GET /threads/{id}/tools`.
pub async fn tools(
    State(db): State<Database>,
    Path(id): Path<i32>,
) -> Result<Json<Vec<tool::Model>>, ApiError> {
    let thread = load(&db, id).await?;
    Ok(Json(threads::tools_of_thread(&db, &thread).await?))
}

/// `GET /threads/using/{tool}`.
pub async fn using(
    State(db): State<Database>,
    Path(tool): Path<String>,
) -> Result<Json<Vec<thread::Model>>, ApiError> {
    Ok(Json(threads::threads_using(&db, &tool).await?))
}

/// `GET /threads/with-tools`.
pub async fn with_tools(
    State(db): State<Database>,
) -> Result<Json<Vec<ThreadWithTools>>, ApiError> {
    Ok(Json(threads::threads_with_tools(&db).await?))
}

/// `GET /threads/summaries`.
pub async fn summaries(State(db): State<Database>) -> Result<Json<Vec<ThreadSummary>>, ApiError> {
    Ok(Json(threads::summaries(&db).await?))
}

/// `GET /threads/titles`.
pub async fn titles(State(db): State<Database>) -> Result<Json<Vec<(i32, String)>>, ApiError> {
    Ok(Json(threads::titles(&db).await?))
}

/// `GET /threads/any-completed`.
pub async fn any_completed(State(db): State<Database>) -> Result<Json<bool>, ApiError> {
    Ok(Json(threads::any_completed(&db).await?))
}
