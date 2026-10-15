//! Routes of the models.

use crate::entity::llm_model;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use turso_orm::Database;

use crate::dto::llm_model::{CreateModel, ModelFilter, ModelStats, ModelWithAgents, Price};
use crate::error::ApiError;
use crate::extract::{ApiJson, ApiPath, ApiQuery};
use crate::service::llm_model as service;

/// `DELETE /models/below`.
pub async fn remove_below(
    State(db): State<Database>,
    ApiJson(body): ApiJson<Price>,
) -> Result<Json<Vec<llm_model::Model>>, ApiError> {
    Ok(Json(
        service::remove_models_below(&db, body.input_price).await?,
    ))
}

/// `GET /models`.
pub async fn search(
    State(db): State<Database>,
    ApiQuery(filter): ApiQuery<ModelFilter>,
) -> Result<Json<Vec<llm_model::Model>>, ApiError> {
    Ok(Json(service::search_models(&db, &filter).await?))
}

/// `GET /models/{id}`.
pub async fn find(
    State(db): State<Database>,
    ApiPath(id): ApiPath<i32>,
) -> Result<Json<llm_model::Model>, ApiError> {
    service::find_model(&db, id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("model"))
}

/// `POST /models`.
pub async fn create(
    State(db): State<Database>,
    ApiJson(body): ApiJson<CreateModel>,
) -> Result<(StatusCode, Json<llm_model::Model>), ApiError> {
    let created = service::create_model(&db, body).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// `POST /models/batch`.
pub async fn create_many(
    State(db): State<Database>,
    ApiJson(bodies): ApiJson<Vec<CreateModel>>,
) -> Result<(StatusCode, Json<Vec<llm_model::Model>>), ApiError> {
    let created = service::create_models(&db, bodies).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// `PUT /models/{id}/price`.
pub async fn update_price(
    State(db): State<Database>,
    ApiPath(id): ApiPath<i32>,
    ApiJson(body): ApiJson<Price>,
) -> Result<Json<llm_model::Model>, ApiError> {
    service::update_price(&db, id, body.input_price)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("model"))
}

/// `PATCH /models/discount`.
pub async fn discount(
    State(db): State<Database>,
    ApiJson(body): ApiJson<Price>,
) -> Result<Json<u64>, ApiError> {
    Ok(Json(
        service::discount_tool_less_models(&db, body.input_price).await?,
    ))
}

/// `GET /models/stats`.
pub async fn stats(State(db): State<Database>) -> Result<Json<ModelStats>, ApiError> {
    service::stats(&db)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("stats"))
}

/// `GET /models/with-agents`.
pub async fn with_agents(
    State(db): State<Database>,
) -> Result<Json<Vec<ModelWithAgents>>, ApiError> {
    Ok(Json(service::models_with_agents(&db).await?))
}
