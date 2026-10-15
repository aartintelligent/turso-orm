//! Routes of the threads.

use crate::entity::Thread;
use crate::entity::thread;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use turso_orm::prelude::*;

use crate::dto::base::pagination::After;
use crate::dto::thread::{
    Fork, OpenThread, OpenWithTools, Purge, ThreadSummary, ThreadWithRecords, ThreadWithTools,
};
use crate::error::ApiError;
use crate::extract::{ApiJson, ApiPath, ApiQuery};
use crate::service::thread as service;

/// The thread named by the path, or `404 Not Found`.
pub(crate) async fn load(db: &Database, id: i32) -> Result<thread::Model, ApiError> {
    Thread::find_by_id(id)
        .one(db)
        .await?
        .ok_or(ApiError::NotFound("thread"))
}

/// `POST /threads`.
pub async fn open(
    State(db): State<Database>,
    ApiJson(body): ApiJson<OpenThread>,
) -> Result<(StatusCode, Json<thread::Model>), ApiError> {
    let opened = service::open_thread(&db, body).await?;
    Ok((StatusCode::CREATED, Json(opened)))
}

/// `POST /threads/json`: a raw object, every key naming a column is set.
pub async fn open_from_json(
    State(db): State<Database>,
    ApiJson(body): ApiJson<serde_json::Value>,
) -> Result<(StatusCode, Json<thread::Model>), ApiError> {
    let opened = service::open_thread_from_json(&db, body).await?;
    Ok((StatusCode::CREATED, Json(opened)))
}

/// `POST /threads/with-tools`.
pub async fn open_with_tools(
    State(db): State<Database>,
    ApiJson(body): ApiJson<OpenWithTools>,
) -> Result<(StatusCode, Json<ThreadWithTools>), ApiError> {
    let opened = service::open_thread_with_tools(&db, body).await?;
    Ok((StatusCode::CREATED, Json(opened)))
}

/// `POST /threads/{id}/complete`.
pub async fn complete(
    State(db): State<Database>,
    ApiPath(id): ApiPath<i32>,
) -> Result<Json<thread::Model>, ApiError> {
    service::complete_thread(&db, id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("thread"))
}

/// `DELETE /threads?prefix=`.
pub async fn purge(
    State(db): State<Database>,
    ApiQuery(purge): ApiQuery<Purge>,
) -> Result<Json<Vec<thread::Model>>, ApiError> {
    Ok(Json(service::purge_threads(&db, &purge.prefix).await?))
}

/// `GET /threads?after=&size=`.
pub async fn list(
    State(db): State<Database>,
    ApiQuery(after): ApiQuery<After>,
) -> Result<Json<Vec<thread::Model>>, ApiError> {
    Ok(Json(
        service::threads_after(&db, after.after, after.size()).await?,
    ))
}

/// `GET /threads/{id}`.
pub async fn get(
    State(db): State<Database>,
    ApiPath(id): ApiPath<i32>,
) -> Result<Json<ThreadWithRecords>, ApiError> {
    service::thread_with_records(&db, id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("thread"))
}

/// `POST /threads/{id}/fork`.
pub async fn fork(
    State(db): State<Database>,
    ApiPath(id): ApiPath<i32>,
    ApiJson(body): ApiJson<Fork>,
) -> Result<(StatusCode, Json<thread::Model>), ApiError> {
    let parent = load(&db, id).await?;
    let forked = service::fork_thread(&db, &parent, body.agent_id).await?;
    Ok((StatusCode::CREATED, Json(forked)))
}

/// `GET /threads/{id}/parent`.
pub async fn parent(
    State(db): State<Database>,
    ApiPath(id): ApiPath<i32>,
) -> Result<Json<thread::Model>, ApiError> {
    let fork = load(&db, id).await?;
    service::parent_of(&db, &fork)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("parent"))
}

/// `GET /threads/{id}/forks`.
pub async fn forks(
    State(db): State<Database>,
    ApiPath(id): ApiPath<i32>,
) -> Result<Json<Vec<thread::Model>>, ApiError> {
    let root = load(&db, id).await?;
    Ok(Json(service::forks_of(&db, &root).await?))
}

/// `GET /threads/reviewed-by/{name}`.
pub async fn reviewed_by(
    State(db): State<Database>,
    ApiPath(name): ApiPath<String>,
) -> Result<Json<Vec<thread::Model>>, ApiError> {
    Ok(Json(service::reviewed_by(&db, &name).await?))
}

/// `GET /threads/summaries`.
pub async fn summaries(State(db): State<Database>) -> Result<Json<Vec<ThreadSummary>>, ApiError> {
    Ok(Json(service::summaries(&db).await?))
}

/// `GET /threads/titles`.
pub async fn titles(State(db): State<Database>) -> Result<Json<Vec<(i32, String)>>, ApiError> {
    Ok(Json(service::titles(&db).await?))
}

/// `GET /threads/any-completed`.
pub async fn any_completed(State(db): State<Database>) -> Result<Json<bool>, ApiError> {
    Ok(Json(service::any_completed(&db).await?))
}
