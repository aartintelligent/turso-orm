//! The migrator of the axum example and its migrations, oldest first.
//!
//! Each migration lives in a module named `m<date>_<seq>_<what>`; the
//! `DeriveMigrationName` derive takes the migration name from that module,
//! so every struct is simply called `Migration`.

pub use turso_orm_migration::prelude::*;

mod m20240101_000001_create_post_table;
mod m20240101_000002_seed_posts;

/// The migrator; `migrations` is the only method to implement.
pub struct Migrator;

#[async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240101_000001_create_post_table::Migration),
            Box::new(m20240101_000002_seed_posts::Migration),
        ]
    }
}
