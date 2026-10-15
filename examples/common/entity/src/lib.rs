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
//! Each entity is a module holding its `Model` and the items the derive
//! generates from it; the prelude re-exports the entity types under short
//! names for callers that only need `find`, `insert` and friends.

pub mod agent;
pub mod llm_model;
pub mod llm_provider;
pub mod prelude;
pub mod thread;
pub mod thread_record;
pub mod thread_tool;
pub mod tool;
