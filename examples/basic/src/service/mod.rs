//! The functions that read and write the database through the ORM.
//!
//! They take a connection and return plain values or `DbErr`; nothing here
//! knows about HTTP. The documentation site includes them block by block,
//! which is why each one stays short and does one thing.

pub mod agents;
pub mod catalog;
pub mod sync;
pub mod threads;
