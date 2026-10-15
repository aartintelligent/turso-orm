//! A JSON API on axum over a small agent runtime schema, in four layers.
//!
//! `entity` holds the schema, one module per table, and `migration` the
//! migrations that create and seed it. `dto` describes what goes in and out
//! as JSON, `service` talks to the
//! database through the ORM and knows nothing of HTTP, `handler` binds the
//! two, `router` lists every route, and `error` maps a `DbErr` to a status.
//! Around them, `settings` reads the database configuration, `db` opens the
//! database in the mode it names, `state` carries the pool and the settings
//! into the handlers, and `telemetry` sets up tracing. The documentation site
//! includes the services, the DTOs, the error mapping, a handler and the
//! connection block by block.

pub mod db;
pub mod dto;
pub mod entity;
pub mod error;
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
