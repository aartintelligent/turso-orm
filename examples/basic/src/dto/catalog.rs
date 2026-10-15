//! Bodies and responses of the catalog of models.

use crate::entity::{agent, llm_model};
use serde::{Deserialize, Serialize};
use turso_orm::prelude::*;

/// The body of `POST /models`, one item of `POST /models/batch`.
#[derive(Debug, Deserialize, DeriveIntoActiveModel)]
#[turso(active_model = "llm_model::ActiveModel")]
pub struct CreateModel {
    pub name: String,
    pub input_price: f64,
    pub supports_tools: bool,
    pub capabilities: Option<Json>,
}

/// The body of `PUT /models/{id}/price`, `PATCH /models/discount` and `DELETE /models/below`.
#[derive(Debug, Deserialize)]
pub struct Price {
    pub input_price: f64,
}

/// `GET /models` parameters: a name fragment, and an `OR` of two criteria.
#[derive(Debug, Default, Deserialize)]
pub struct ModelFilter {
    /// Part of the name.
    pub name: Option<String>,
    /// Keep the models that can call tools.
    pub tools: Option<bool>,
    /// Keep the models priced above this.
    pub min_price: Option<f64>,
}

// --8<-- [start:project_struct]
/// Aggregates over the models decoded into a plain struct.
#[derive(Debug, Serialize, FromQueryResult)]
pub struct ModelStats {
    /// Number of models.
    pub count: i64,
    /// Highest price per million input tokens; `NULL`, hence `None`, on an empty table.
    #[turso(column_name = "max_price")]
    pub most_expensive: Option<f64>,
}
// --8<-- [end:project_struct]

/// A model with the agents configured on it.
#[derive(Debug, Serialize)]
pub struct ModelWithAgents {
    pub model: llm_model::Model,
    pub agents: Vec<agent::Model>,
}
