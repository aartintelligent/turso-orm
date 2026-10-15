//! The handlers: one short function per route, each binding the extractors
//! of axum to a service call and letting `ApiError` turn a `DbErr` into a
//! status. Nothing here touches the ORM directly.

pub mod agents;
pub mod catalog;
pub mod sync;
pub mod threads;
