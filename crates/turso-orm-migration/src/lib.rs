//! Migration: versioned schema migrations for turso-orm.
//!
//! A migration is a type implementing [`MigrationTrait`] (with `up` and
//! `down`) and [`MigrationName`] (derived with [`DeriveMigrationName`] from
//! the module name). A [`MigratorTrait`] lists the migrations in order and
//! applies them through a [`SchemaManager`], which wraps the transaction a
//! migration runs in and offers DDL helpers and catalog lookups.
//!
//! The crate owns the bookkeeping — which migrations have been applied, in
//! which order — and the transactional envelope around each one. It does not
//! own DDL rendering (that is `turso_sql`, re-exported through the prelude)
//! nor the connection (that is `turso_orm`).
//!
//! # Design decisions
//!
//! - Each migration runs together with its bookkeeping insert inside one
//!   `BEGIN IMMEDIATE` transaction, so a failing migration leaves neither a
//!   partial schema nor a stale version row behind.
//! - Versions are the migration names, taken from the module path, so that
//!   every migration struct can simply be called `Migration` and the file
//!   name is the single source of truth for ordering.
//! - `down` defaults to failing with [`DbErr::Migration`], so a migration
//!   that cannot be reverted says so instead of silently doing nothing.
//!
//! # Example
//!
//! ```ignore
//! use turso_orm_migration::prelude::*;
//!
//! mod m20240101_000001_create_user {
//!     use turso_orm_migration::prelude::*;
//!
//!     #[derive(DeriveMigrationName)]
//!     pub struct Migration;
//!
//!     #[async_trait]
//!     impl MigrationTrait for Migration {
//!         async fn up(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
//!             manager
//!                 .create_table(
//!                     Table::create()
//!                         .table("user")
//!                         .col(ColumnDef::integer("id").primary_key().auto_increment())
//!                         .col(ColumnDef::text("email").not_null().unique_key()),
//!                 )
//!                 .await
//!         }
//!
//!         async fn down(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
//!             manager.drop_table(Table::drop().table("user")).await
//!         }
//!     }
//! }
//!
//! pub struct Migrator;
//!
//! #[async_trait]
//! impl MigratorTrait for Migrator {
//!     fn migrations() -> Vec<Box<dyn MigrationTrait>> {
//!         vec![Box::new(m20240101_000001_create_user::Migration)]
//!     }
//! }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]

mod manager;
mod migrator;

pub use manager::SchemaManager;
pub use migrator::{MigrationStatus, MigratorTrait};

pub use async_trait::async_trait;
pub use turso_orm;
pub use turso_orm::DbErr;
pub use turso_orm_macros::DeriveMigrationName;

/// The name of a migration, used as its version key in the bookkeeping table.
pub trait MigrationName {
    /// The name, which must be unique across the migrator.
    fn name(&self) -> &str;
}

/// A migration: a reversible schema change.
#[async_trait]
pub trait MigrationTrait: MigrationName + Send + Sync {
    /// Applies the migration inside the transaction `manager` wraps.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when a statement fails; implementations may
    /// return any other variant to abort, and the transaction is rolled back.
    async fn up(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr>;

    /// Reverts the migration inside the transaction `manager` wraps.
    ///
    /// # Errors
    ///
    /// The default returns [`DbErr::Migration`] unconditionally, marking the
    /// migration as irreversible; overriding implementations return
    /// [`DbErr::Driver`] when a statement fails.
    async fn down(&self, _manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        Err(DbErr::Migration("this migration cannot be reverted".into()))
    }
}

/// Everything a migration module needs, meant to be glob-imported.
pub mod prelude {
    pub use crate::{
        DeriveMigrationName, MigrationName, MigrationStatus, MigrationTrait, MigratorTrait,
        SchemaManager, async_trait,
    };
    pub use turso_orm::entity::EntityTrait;
    pub use turso_orm::sql::{
        AlterTable, ColumnDef, ColumnType, CreateIndex, CreateTable, DropIndex, DropTable, Expr,
        ForeignKey, ForeignKeyAction, Order, Table,
    };
    pub use turso_orm::{
        ConnectOptions, ConnectionTrait, Database, DbErr, Schema, Statement, Transaction,
        TransactionMode, TransactionTrait,
    };
}
