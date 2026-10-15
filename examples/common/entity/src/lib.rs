//! The entities of the agent runtime that every example shares.
//!
//! The runtime, as the examples picture it: a provider serves language
//! models; an agent is configured on one of them; an agent holds threads;
//! a thread is a sequence of records, the prompts, the answers and the tool
//! calls; and the tools a thread invoked are kept through a junction. A
//! thread may also be forked from another one and reviewed by a second
//! agent, which is what gives the schema a self-reference and two relations
//! to the same table.
//!
//! ```text
//! llm_provider 1--* llm_model 1--* agent 1--* thread 1--* thread_record
//!                                             thread *--* tool, through thread_tool
//!                                             thread 1--* thread, a fork and its parent
//! ```
//!
//! Each entity is a module under `resource`, holding its `Model` and the
//! items the derive generates from it, and re-exported here so that callers
//! write `entity::thread`; the prelude re-exports the entity types under
//! short names for callers that only need `find`, `insert` and friends. The
//! models derive `Serialize` and `Deserialize`, so an API can hand them out
//! and take them in as JSON directly.

pub mod prelude;
pub mod resource;

pub use resource::{agent, llm_model, llm_provider, thread, thread_record, thread_tool, tool};
