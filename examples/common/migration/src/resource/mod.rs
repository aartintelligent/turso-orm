//! One migration per table, then the seeds, in the order the migrator applies them.

pub mod m20240101_000001_create_llm_provider;
pub mod m20240101_000002_create_llm_model;
pub mod m20240101_000003_create_agent;
pub mod m20240101_000004_create_thread;
pub mod m20240101_000005_create_thread_record;
pub mod m20240101_000006_create_tool;
pub mod m20240101_000007_create_thread_tool;
pub mod m20240101_000008_seed_llm_provider;
pub mod m20240101_000009_seed_llm_model;
