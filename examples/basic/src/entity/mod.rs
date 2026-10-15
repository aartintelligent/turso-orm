//! The agent runtime schema: one module per table, and short names for the entities.
//!
//! A model is configured on agents; an agent holds threads; a thread is a
//! sequence of records, the prompts, the answers and the tool calls; and
//! the tools a thread invoked are kept through a junction. A thread may be
//! forked from another one and reviewed by a second agent, which is what
//! gives the schema a self-reference and two relations to the same table.
//!
//! ```text
//! llm_model 1--* agent 1--* thread 1--* thread_record
//!                            thread *--* tool, through thread_tool
//!                            thread 1--* thread, a fork and its parent
//! ```
//!
//! Each entity is a module holding its `Model` and the items the derive
//! generates from it. The models derive `Serialize` and `Deserialize`, so
//! the API hands them out and takes them in as JSON directly.

pub mod agent;
pub mod llm_model;
pub mod thread;
pub mod thread_record;
pub mod thread_tool;
pub mod tool;

pub use agent::Entity as Agent;
pub use llm_model::Entity as LlmModel;
pub use thread::Entity as Thread;
pub use thread_record::Entity as ThreadRecord;
pub use thread_tool::Entity as ThreadTool;
pub use tool::Entity as Tool;
