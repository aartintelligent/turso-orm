//! The handlers: one short function per route, one module per table, each
//! binding the extractors to a service call and letting `ApiError` turn a
//! `DbErr` into a status. Nothing here touches the ORM directly.

pub mod agent;
pub mod health;
pub mod llm_model;
pub mod sync;
pub mod thread;
pub mod thread_record;
pub mod thread_tool;
