//! Bodies and responses of the agents.

use crate::entity::{agent, llm_model, tool};
use serde::{Deserialize, Serialize};
use turso_orm::prelude::*;

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

/// `GET /agents/with-models` parameters.
#[derive(Debug, Default, Deserialize)]
pub struct AgentFilter {
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
