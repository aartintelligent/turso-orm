//! What the API takes in and hands out as JSON: request bodies, query
//! parameters and response shapes, one module per table plus `base` for
//! what every listing shares. The models of `entity` are used directly
//! where a response is exactly a row.

pub mod agent;
pub mod base;
pub mod llm_model;
pub mod sync;
pub mod thread;
pub mod thread_record;
pub mod thread_tool;
