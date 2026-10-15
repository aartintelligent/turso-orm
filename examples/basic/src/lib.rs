//! A JSON API on axum over a small agent runtime schema, in four layers.
//!
//! `entity` holds the schema, one module per table, and `migration` the
//! migrations that create and seed it. Then one module per table in each
//! layer, so that a table's name finds every piece of it: `dto` describes
//! what goes in and out as JSON, `service` talks to the database through
//! the ORM on any connection and knows nothing of HTTP, `handler` binds the
//! two through the `extract` wrappers, `router` spells every path, and
//! `error` maps a `DbErr` and a refused request to a status.
//! Around them, `settings` reads the database configuration, `db` opens the
//! database in the mode it names, `state` carries the pool and the settings
//! into the handlers, and `telemetry` sets up tracing. In the `sync` mode
//! the database is an embedded replica and a `ReplicaSync` task, started by
//! `main` and carried by the state, pushes and pulls it on a schedule;
//! `POST /sync` forces a round. The documentation site includes the services, the
//! DTOs, the error mapping, a handler and the connection block by block.

pub mod db;
pub mod dto;
pub mod entity;
pub mod error;
pub mod extract;
pub mod handler;
pub mod migration;
pub mod router;
pub mod service;
pub mod settings;
pub mod state;
pub mod telemetry;

use axum::Router;
use tower_http::trace::TraceLayer;

pub use state::AppState;

/// The router of the API over `state`, with one tracing span per request.
pub fn app(state: AppState) -> Router {
    router::build(state).layer(TraceLayer::new_for_http())
}
