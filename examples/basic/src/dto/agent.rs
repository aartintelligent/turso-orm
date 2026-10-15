//! Bodies, parameters and responses of the agents.

use crate::entity::{agent, llm_model, tool};
use serde::{Deserialize, Serialize};
use turso_orm::prelude::*;

use crate::dto::base::pagination::{Direction, PageRequest};
use crate::service::agent::{AgentFilter, AgentQuery, AgentSort};

/// The body of `POST /agents`: a subset of the columns; the rest take the defaults.
#[derive(Debug, Deserialize, DeriveIntoActiveModel)]
#[turso(active_model = "agent::ActiveModel")]
pub struct CreateAgent {
    pub name: String,
    pub system_prompt: Option<String>,
    pub model_id: Option<i32>,
}

/// The body of `PUT /agents`: with an `id` the agent is updated, without it is created.
#[derive(Debug, Deserialize)]
pub struct SaveAgent {
    pub id: Option<i32>,
    pub name: String,
}

// --8<-- [start:list_params]
/// `GET /agents` parameters: what to keep, how to order, which page.
///
/// The wire spelling of the sort is pinned here; the service's `AgentSort`
/// is what the query is built from.
#[derive(Debug, Default, Deserialize)]
pub struct ListAgents {
    /// Keep the agents whose name contains this.
    pub name: Option<String>,
    /// Keep the agents configured on this model.
    pub model_id: Option<i32>,
    /// The column to order by.
    #[serde(default)]
    pub sort: AgentSortParam,
    /// The direction of the ordering.
    #[serde(default)]
    pub direction: Direction,
    /// The page to return, counted from zero.
    pub page: Option<u64>,
    /// Items per page.
    pub size: Option<u64>,
}

/// `?sort=`: the columns a listing of agents can be ordered by.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentSortParam {
    /// By key, the insertion order.
    #[default]
    Id,
    /// Alphabetically.
    Name,
}

impl From<ListAgents> for AgentQuery {
    fn from(params: ListAgents) -> Self {
        Self {
            filter: AgentFilter {
                name: params.name,
                model_id: params.model_id,
            },
            sort: match params.sort {
                AgentSortParam::Id => AgentSort::Id,
                AgentSortParam::Name => AgentSort::Name,
            },
            direction: params.direction.into(),
            page: PageRequest::new(params.page, params.size),
        }
    }
}
// --8<-- [end:list_params]

/// `GET /agents/with-models` parameters.
#[derive(Debug, Default, Deserialize)]
pub struct AgentsWithModels {
    /// Keep the agents whose model is priced above this.
    pub min_model_price: Option<f64>,
}

/// An agent with the model it runs on, if any.
#[derive(Debug, Serialize)]
pub struct AgentWithModel {
    pub agent: agent::Model,
    pub model: Option<llm_model::Model>,
}

/// An agent with the tools its threads invoked.
#[derive(Debug, Serialize)]
pub struct AgentWithTools {
    pub agent: agent::Model,
    pub tools: Vec<tool::Model>,
}
