//! Applying and reverting migrations, modeled by [`MigratorTrait`].
//!
//! The migrator keeps a bookkeeping table, `turso_migrations` unless
//! [`MigratorTrait::migration_table_name`] says otherwise, with one row per
//! applied migration. Every operation starts by making sure that table
//! exists and reading it, then walks the declared migrations in order (or in
//! reverse for `down`) and skips the ones whose state already matches.
//!
//! Each migration runs inside its own `BEGIN IMMEDIATE` transaction, and
//! the bookkeeping insert or delete is issued on that same transaction
//! before the commit. Taking the write lock up front avoids a busy error
//! mid-migration, and bundling the version row with the schema change means
//! a failure leaves neither a partial schema nor a misleading version row.

use async_trait::async_trait;
use turso_orm::sql::{ColumnDef, Expr, Order, Query, Table};
use turso_orm::{ConnectionTrait, Database, DbErr, Statement, TransactionMode, TransactionTrait};

use crate::MigrationTrait;
use crate::manager::{SchemaManager, has_table};

/// The default name of the bookkeeping table.
const DEFAULT_TABLE: &str = "turso_migrations";

/// The status of one declared migration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrationStatus {
    /// The migration name.
    pub name: String,
    /// Whether the migration has been applied.
    pub applied: bool,
}

/// Lists migrations and applies or reverts them in order.
///
/// Implement [`migrations`](Self::migrations) only; every other method has
/// a default built on it.
#[async_trait]
pub trait MigratorTrait: Send {
    /// Every migration, oldest first.
    fn migrations() -> Vec<Box<dyn MigrationTrait>>;

    /// The name of the bookkeeping table, `turso_migrations` by default.
    ///
    /// Override it to run several migrators against one database, or to
    /// keep the table name of a schema that was migrated by another tool.
    fn migration_table_name() -> &'static str {
        DEFAULT_TABLE
    }

    /// Creates the bookkeeping table if it does not exist yet.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the statement fails.
    async fn install(db: &Database) -> Result<(), DbErr> {
        let stmt = Table::create()
            .table(Self::migration_table_name())
            .if_not_exists()
            .col(ColumnDef::text("version").primary_key().not_null())
            .col(ColumnDef::integer("applied_at").not_null());
        db.execute(turso_orm::Build::to_statement(&stmt)).await?;
        Ok(())
    }

    /// The names of the applied migrations, in application order.
    ///
    /// Returns an empty list when the bookkeeping table does not exist, so
    /// that status can be queried on a database that was never migrated.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Migration`] when the catalog query returns no row;
    /// [`DbErr::Driver`] when a query fails or a version cannot be decoded.
    async fn get_applied_migrations(db: &Database) -> Result<Vec<String>, DbErr> {
        if !has_table(db, Self::migration_table_name()).await? {
            return Ok(Vec::new());
        }
        // The timestamp has second resolution, so the name breaks ties
        // between migrations applied within the same second.
        let stmt = Query::select()
            .column("version")
            .from(Self::migration_table_name())
            .order_by("applied_at", Order::Asc)
            .order_by("version", Order::Asc);
        let rows = db.query_all(turso_orm::Build::to_statement(&stmt)).await?;
        rows.iter()
            .map(|r| r.get::<String>("version").map_err(DbErr::from))
            .collect()
    }

    /// The status of every declared migration, in declaration order.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`get_applied_migrations`](Self::get_applied_migrations).
    async fn status(db: &Database) -> Result<Vec<MigrationStatus>, DbErr> {
        let applied = Self::get_applied_migrations(db).await?;
        Ok(Self::migrations()
            .iter()
            .map(|m| MigrationStatus {
                name: m.name().to_owned(),
                applied: applied.iter().any(|a| a == m.name()),
            })
            .collect())
    }

    /// Applies the pending migrations, all of them or the first `steps`.
    ///
    /// Each migration and its version row are committed together; on the
    /// first failure the transaction is dropped and rolled back, and the
    /// error is returned without touching later migrations.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when a statement or the transaction fails;
    /// any error the migration's `up` returns.
    async fn up(db: &Database, steps: Option<u32>) -> Result<(), DbErr> {
        Self::install(db).await?;
        let applied = Self::get_applied_migrations(db).await?;
        let mut remaining = steps.map_or(usize::MAX, |s| s as usize);
        for migration in Self::migrations() {
            if remaining == 0 {
                break;
            }
            if applied.iter().any(|a| a == migration.name()) {
                continue;
            }
            tracing::info!(name = migration.name(), "applying migration");
            // `IMMEDIATE` takes the write lock now rather than at the first
            // write, so the migration cannot hit a busy error halfway.
            let txn = db.begin_with_mode(TransactionMode::Immediate).await?;
            {
                let manager = SchemaManager::new(&txn);
                migration.up(&manager).await?;
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
                .unwrap_or_default();
            let insert = Query::insert()
                .into_table(Self::migration_table_name())
                .columns(["version", "applied_at"])
                .values([Expr::val(migration.name()), Expr::val(now)]);
            txn.execute(turso_orm::Build::to_statement(&insert)).await?;
            txn.commit().await?;
            remaining -= 1;
        }
        Ok(())
    }

    /// Reverts the applied migrations, newest first, all of them or `steps`.
    ///
    /// Each migration's `down` and the deletion of its version row are
    /// committed together, mirroring [`up`](Self::up).
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when a statement or the transaction fails;
    /// any error the migration's `down` returns, including the default
    /// [`DbErr::Migration`] of an irreversible migration.
    async fn down(db: &Database, steps: Option<u32>) -> Result<(), DbErr> {
        Self::install(db).await?;
        let applied = Self::get_applied_migrations(db).await?;
        let mut remaining = steps.map_or(usize::MAX, |s| s as usize);
        for migration in Self::migrations().into_iter().rev() {
            if remaining == 0 {
                break;
            }
            if !applied.iter().any(|a| a == migration.name()) {
                continue;
            }
            tracing::info!(name = migration.name(), "reverting migration");
            let txn = db.begin_with_mode(TransactionMode::Immediate).await?;
            {
                let manager = SchemaManager::new(&txn);
                migration.down(&manager).await?;
            }
            let delete = Query::delete()
                .from_table(Self::migration_table_name())
                .and_where(Expr::col("version").eq(Expr::val(migration.name())));
            txn.execute(turso_orm::Build::to_statement(&delete)).await?;
            txn.commit().await?;
            remaining -= 1;
        }
        Ok(())
    }

    /// Drops every user table, including the bookkeeping table, then applies all migrations.
    ///
    /// Internal `sqlite_*` and `__turso_*` tables are left alone.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when a query, a drop or the transaction
    /// fails; the errors of [`up`](Self::up).
    async fn fresh(db: &Database) -> Result<(), DbErr> {
        let rows = db
            .query_all(Statement::from_string(
                "SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '__turso_%'",
            ))
            .await?;
        let txn = db.begin_with_mode(TransactionMode::Immediate).await?;
        // `PRAGMA foreign_keys` has no effect inside a transaction, so a
        // table referenced by another one may refuse to drop first. Drop in
        // rounds, retrying the tables that failed, until nothing is left or a
        // round makes no progress — then the last error is the real one.
        let mut pending: Vec<String> = rows
            .iter()
            .map(|row| row.get::<String>("name"))
            .collect::<Result<_, _>>()?;
        while !pending.is_empty() {
            let before = pending.len();
            let mut failed = Vec::new();
            let mut last_error = None;
            for name in pending {
                let drop = Table::drop().table(name.clone()).if_exists();
                if let Err(err) = txn.execute(turso_orm::Build::to_statement(&drop)).await {
                    failed.push(name);
                    last_error = Some(err);
                }
            }
            if failed.len() == before
                && let Some(err) = last_error
            {
                return Err(err.into());
            }
            pending = failed;
        }
        txn.commit().await?;
        Self::up(db, None).await
    }

    /// Reverts every migration, then applies every migration.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`down`](Self::down) and [`up`](Self::up).
    async fn refresh(db: &Database) -> Result<(), DbErr> {
        Self::down(db, None).await?;
        Self::up(db, None).await
    }

    /// Reverts every migration.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`down`](Self::down).
    async fn reset(db: &Database) -> Result<(), DbErr> {
        Self::down(db, None).await
    }
}
