//! Routes of the catalog of models.

use crate::entity::llm_model;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use turso_orm::Database;

use crate::dto::catalog::{CreateModel, ModelFilter, ModelStats, ModelWithAgents, Price};
use crate::error::ApiError;
use crate::service::catalog;

/// `DELETE /models/below`.
pub async fn remove_below(
    State(db): State<Database>,
    Json(body): Json<Price>,
) -> Result<Json<Vec<llm_model::Model>>, ApiError> {
    Ok(Json(
        catalog::remove_models_below(&db, body.input_price).await?,
    ))
}

/// `GET /models`.
pub async fn search_models(
    State(db): State<Database>,
    Query(filter): Query<ModelFilter>,
) -> Result<Json<Vec<llm_model::Model>>, ApiError> {
    Ok(Json(catalog::search_models(&db, &filter).await?))
}

/// `GET /models/{id}`.
pub async fn find_model(
    State(db): State<Database>,
    Path(id): Path<i32>,
) -> Result<Json<llm_model::Model>, ApiError> {
    catalog::find_model(&db, id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("model"))
}

/// `POST /models`.
pub async fn create_model(
    State(db): State<Database>,
    Json(body): Json<CreateModel>,
) -> Result<(StatusCode, Json<llm_model::Model>), ApiError> {
    let created = catalog::create_model(&db, body).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// `POST /models/batch`.
pub async fn create_models(
    State(db): State<Database>,
    Json(bodies): Json<Vec<CreateModel>>,
) -> Result<(StatusCode, Json<Vec<llm_model::Model>>), ApiError> {
    let created = catalog::create_models(&db, bodies).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// `PUT /models/{id}/price`.
pub async fn update_price(
    State(db): State<Database>,
    Path(id): Path<i32>,
    Json(body): Json<Price>,
) -> Result<Json<llm_model::Model>, ApiError> {
    catalog::update_price(&db, id, body.input_price)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("model"))
}

/// `PATCH /models/discount`.
pub async fn discount(
    State(db): State<Database>,
    Json(body): Json<Price>,
) -> Result<Json<u64>, ApiError> {
    Ok(Json(
        catalog::discount_tool_less_models(&db, body.input_price).await?,
    ))
}

/// `GET /models/stats`.
pub async fn stats(State(db): State<Database>) -> Result<Json<ModelStats>, ApiError> {
    catalog::stats(&db)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("stats"))
}

/// `GET /models/with-agents`.
pub async fn with_agents(
    State(db): State<Database>,
) -> Result<Json<Vec<ModelWithAgents>>, ApiError> {
    Ok(Json(catalog::models_with_agents(&db).await?))
}
