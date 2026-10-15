//! The migrator of the agent runtime and its migrations, oldest first.
//!
//! Each migration lives in a module named `m<date>_<seq>_<what>`; the
//! `DeriveMigrationName` derive takes the migration name from that module,
//! so every struct is simply called `Migration`.

pub use turso_orm_migration::prelude::*;

mod m20240101_000001_create_llm_tables;
mod m20240101_000002_create_agent_table;
mod m20240101_000003_create_thread_tables;
mod m20240101_000004_seed_catalog;

/// The migrator; `migrations` is the only method to implement.
pub struct Migrator;

#[async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240101_000001_create_llm_tables::Migration),
            Box::new(m20240101_000002_create_agent_table::Migration),
            Box::new(m20240101_000003_create_thread_tables::Migration),
            Box::new(m20240101_000004_seed_catalog::Migration),
        ]
    }
}
