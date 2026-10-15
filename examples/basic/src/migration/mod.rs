//! The migrator and its migrations, oldest first.
//!
//! Each migration lives in a module named `m<date>_<seq>_<what>` and
//! creates or seeds exactly one table; the `DeriveMigrationName` derive
//! takes the migration name from that module, so every struct is simply
//! called `Migration`.

pub use turso_orm_migration::prelude::*;

mod m20240101_000001_create_llm_model;
mod m20240101_000002_create_agent;
mod m20240101_000003_create_thread;
mod m20240101_000004_create_thread_record;
mod m20240101_000005_create_tool;
mod m20240101_000006_create_thread_tool;
mod m20240101_000007_seed_llm_model;

/// The migrator; `migrations` is the only method to implement.
pub struct Migrator;

#[async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240101_000001_create_llm_model::Migration),
            Box::new(m20240101_000002_create_agent::Migration),
            Box::new(m20240101_000003_create_thread::Migration),
            Box::new(m20240101_000004_create_thread_record::Migration),
            Box::new(m20240101_000005_create_tool::Migration),
            Box::new(m20240101_000006_create_thread_tool::Migration),
            Box::new(m20240101_000007_seed_llm_model::Migration),
        ]
    }
}
