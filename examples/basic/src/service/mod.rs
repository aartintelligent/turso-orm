//! The functions that read and write the database through the ORM, one
//! module per table.
//!
//! They take any connection, the pool or a transaction of the caller, and
//! return plain values or `DbErr`; nothing here knows about HTTP. The
//! documentation site includes them block by block, which is why each one
//! stays short and does one thing.

pub mod agent;
pub mod llm_model;
pub mod sync;
pub mod thread;
pub mod thread_record;
pub mod thread_tool;
