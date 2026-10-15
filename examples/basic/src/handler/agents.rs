//! Routes of the agents.

use crate::entity::{agent, tool};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use turso_orm::Database;

use crate::dto::agent::{AgentFilter, AgentWithModel, AgentWithTools, CreateAgent, SaveAgent};
use crate::dto::page::{Page, Paged};
use crate::error::ApiError;
use crate::service::agents;

// --8<-- [start:handlers]
/// `GET /agents?page=&size=`: a page of agents with their models and the totals.
pub async fn list(
    State(db): State<Database>,
    Query(page): Query<Page>,
) -> Result<Json<Paged<AgentWithModel>>, ApiError> {
    Ok(Json(agents::list_agents(&db, page).await?))
}

/// `POST /agents`: the body is a subset of the columns; a duplicate name
/// comes back as `409 Conflict` through `ApiError`.
pub async fn create(
    State(db): State<Database>,
    Json(body): Json<CreateAgent>,
) -> Result<(StatusCode, Json<agent::Model>), ApiError> {
    let created = agents::create_agent(&db, body).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// `GET /agents/{id}`: the agent with its model, or `404 Not Found`.
pub async fn get(
    State(db): State<Database>,
    Path(id): Path<i32>,
) -> Result<Json<AgentWithModel>, ApiError> {
    agents::get_agent(&db, id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("agent"))
}
// --8<-- [end:handlers]

/// `PUT /agents`.
pub async fn save(
    State(db): State<Database>,
    Json(body): Json<SaveAgent>,
) -> Result<Json<agent::Model>, ApiError> {
    Ok(Json(agents::save_agent(&db, body).await?))
}

/// `DELETE /agents/{id}`.
pub async fn delete(
    State(db): State<Database>,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    match agents::delete_agent(&db, id).await? {
        0 => Err(ApiError::NotFound("agent")),
        _ => Ok(StatusCode::NO_CONTENT),
    }
}

/// `GET /agents/names`.
pub async fn names(State(db): State<Database>) -> Result<Json<Vec<String>>, ApiError> {
    Ok(Json(agents::agent_names(&db).await?))
}

/// `GET /agents/with-models?min_model_price=`.
pub async fn with_models(
    State(db): State<Database>,
    Query(filter): Query<AgentFilter>,
) -> Result<Json<Vec<AgentWithModel>>, ApiError> {
    match filter.min_model_price {
        Some(min_price) => {
            let on_pricey = agents::agents_on_models_above(&db, min_price).await?;
            Ok(Json(
                on_pricey
                    .into_iter()
                    .map(|agent| AgentWithModel { agent, model: None })
                    .collect(),
            ))
        }
        None => Ok(Json(agents::agents_with_models(&db).await?)),
    }
}

/// `GET /agents/with-tools`.
pub async fn with_tools(State(db): State<Database>) -> Result<Json<Vec<AgentWithTools>>, ApiError> {
    Ok(Json(agents::agents_with_tools(&db).await?))
}

/// `GET /agents/{id}/tools`.
pub async fn tools(
    State(db): State<Database>,
    Path(id): Path<i32>,
) -> Result<Json<Vec<tool::Model>>, ApiError> {
    agents::tools_of_agent(&db, id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("agent"))
}
