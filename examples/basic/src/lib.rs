//! A JSON API on axum over the agent runtime schema, in four layers.
//!
//! `dto` describes what goes in and out as JSON, `service` talks to the
//! database through the ORM and knows nothing of HTTP, `handler` binds the
//! two, `router` lists every route, and `error` maps a `DbErr` to a status.
//! The documentation site includes the services, the DTOs, the error mapping
//! and a handler block by block.

pub mod dto;
pub mod error;
pub mod handler;
pub mod router;
pub mod service;

use axum::Router;
use turso_orm::Database;

/// The router of the API over `db`.
pub fn app(db: Database) -> Router {
    router::build(db)
}
