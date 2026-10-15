//! Routes of the agents.

use crate::entity::{agent, tool};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use turso_orm::Database;

use crate::dto::agent::{
    AgentWithModel, AgentWithTools, AgentsWithModels, CreateAgent, ListAgents, SaveAgent,
};
use crate::dto::base::pagination::Page;
use crate::error::ApiError;
use crate::extract::{ApiJson, ApiPath, ApiQuery};
use crate::service::agent as service;

// --8<-- [start:handlers]
/// `GET /agents?name=&model_id=&sort=&direction=&page=&size=`: a page of
/// agents with their models and the totals.
pub async fn list(
    State(db): State<Database>,
    ApiQuery(params): ApiQuery<ListAgents>,
) -> Result<Json<Page<AgentWithModel>>, ApiError> {
    Ok(Json(service::list_agents(&db, &params.into()).await?))
}

/// `POST /agents`: the body is a subset of the columns; a duplicate name
/// comes back as `409 Conflict` through `ApiError`.
pub async fn create(
    State(db): State<Database>,
    ApiJson(body): ApiJson<CreateAgent>,
) -> Result<(StatusCode, Json<agent::Model>), ApiError> {
    let created = service::create_agent(&db, body).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// `GET /agents/{id}`: the agent with its model, or `404 Not Found`.
pub async fn get(
    State(db): State<Database>,
    ApiPath(id): ApiPath<i32>,
) -> Result<Json<AgentWithModel>, ApiError> {
    service::get_agent(&db, id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("agent"))
}
// --8<-- [end:handlers]

/// `PUT /agents`.
pub async fn save(
    State(db): State<Database>,
    ApiJson(body): ApiJson<SaveAgent>,
) -> Result<Json<agent::Model>, ApiError> {
    Ok(Json(service::save_agent(&db, body).await?))
}

/// `DELETE /agents/{id}`.
pub async fn delete(
    State(db): State<Database>,
    ApiPath(id): ApiPath<i32>,
) -> Result<StatusCode, ApiError> {
    match service::delete_agent(&db, id).await? {
        0 => Err(ApiError::NotFound("agent")),
        _ => Ok(StatusCode::NO_CONTENT),
    }
}

/// `GET /agents/names`.
pub async fn names(State(db): State<Database>) -> Result<Json<Vec<String>>, ApiError> {
    Ok(Json(service::agent_names(&db).await?))
}

/// `GET /agents/with-models?min_model_price=`.
pub async fn with_models(
    State(db): State<Database>,
    ApiQuery(params): ApiQuery<AgentsWithModels>,
) -> Result<Json<Vec<AgentWithModel>>, ApiError> {
    match params.min_model_price {
        Some(min_price) => {
            let on_pricey = service::agents_on_models_above(&db, min_price).await?;
            Ok(Json(
                on_pricey
                    .into_iter()
                    .map(|agent| AgentWithModel { agent, model: None })
                    .collect(),
            ))
        }
        None => Ok(Json(service::agents_with_models(&db).await?)),
    }
}

/// `GET /agents/with-tools`.
pub async fn with_tools(State(db): State<Database>) -> Result<Json<Vec<AgentWithTools>>, ApiError> {
    Ok(Json(service::agents_with_tools(&db).await?))
}

/// `GET /agents/{id}/tools`.
pub async fn tools(
    State(db): State<Database>,
    ApiPath(id): ApiPath<i32>,
) -> Result<Json<Vec<tool::Model>>, ApiError> {
    service::tools_of_agent(&db, id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("agent"))
}
