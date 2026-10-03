//! Applying and reverting migrations, modeled by [`MigratorTrait`].
//!
//! The migrator keeps a bookkeeping table, `turso_migrations` unless
//! [`MigratorTrait::migration_table_name`] says otherwise, with one row per
//! applied migration. Every operation starts by making sure that table
//! exists and reading it, then walks the declared migrations in order (or in
//! reverse for `down`) and skips the ones whose state already matches. The
//! state is checked again once each migration holds the write lock, so two
//! migrators started together never run the same migration twice.
//!
//! Each migration runs inside its own `BEGIN IMMEDIATE` transaction, and
//! the bookkeeping insert or delete is issued on that same transaction
//! before the commit. Taking the write lock up front avoids a busy error
//! mid-migration, and bundling the version row with the schema change means
//! a failure leaves neither a partial schema nor a misleading version row.
//!
//! The declared list is validated before anything runs. A duplicate name is
//! always an error, since the bookkeeping table could not tell the two
//! apart. A migration recorded as applied but no longer declared, or a
//! pending one declared before an applied one, is a [`MigrationIssue`]:
//! legitimate after a deployment is rolled back or two branches are merged,
//! so `up` only warns about it unless [`MigratorTrait::strict`] says
//! otherwise.

use std::collections::HashSet;
use std::fmt;

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

/// A mismatch between the declared migrations and the bookkeeping table,
/// reported by [`MigratorTrait::check`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MigrationIssue {
    /// A migration recorded as applied that [`MigratorTrait::migrations`]
    /// no longer declares, for example after a deployment was rolled back
    /// to an older binary or a migration was renamed.
    Unknown(String),
    /// A pending migration declared before one that is already applied,
    /// typically after two branches that each added a migration were
    /// merged; `up` applies it after the later one.
    OutOfOrder(String),
}

impl fmt::Display for MigrationIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(name) => write!(f, "applied migration `{name}` is not declared"),
            Self::OutOfOrder(name) => write!(
                f,
                "pending migration `{name}` is declared before an applied one"
            ),
        }
    }
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

    /// Whether a [`MigrationIssue`] makes [`up`](Self::up) fail rather than
    /// warn; `false` by default.
    ///
    /// Leave it off where an older binary may run against a database that
    /// a newer one migrated, which leaves unknown migrations behind.
    fn strict() -> bool {
        false
    }

    /// Lists the mismatches between the declared migrations and the
    /// bookkeeping table, without changing anything.
    ///
    /// Unknown migrations come first, in application order, then
    /// out-of-order ones, in declaration order.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Migration`] when two declared migrations share a
    /// name; the errors of
    /// [`get_applied_migrations`](Self::get_applied_migrations).
    async fn check(db: &Database) -> Result<Vec<MigrationIssue>, DbErr> {
        let migrations = Self::migrations();
        ensure_unique(&migrations)?;
        let applied = Self::get_applied_migrations(db).await?;
        Ok(find_issues(&migrations, &applied))
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
    /// Returns [`DbErr::Migration`] when two declared migrations share a
    /// name; the errors of
    /// [`get_applied_migrations`](Self::get_applied_migrations).
    async fn status(db: &Database) -> Result<Vec<MigrationStatus>, DbErr> {
        let migrations = Self::migrations();
        ensure_unique(&migrations)?;
        let applied = Self::get_applied_migrations(db).await?;
        Ok(migrations
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
    /// error is returned without touching later migrations. Every
    /// [`MigrationIssue`] is logged as a warning first, or fails the call
    /// when [`strict`](Self::strict) is set.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Migration`] when two declared migrations share a
    /// name, or when [`strict`](Self::strict) is set and
    /// [`check`](Self::check) finds an issue, in both cases before anything
    /// runs; [`DbErr::Driver`] when a statement or the transaction fails;
    /// any error the migration's `up` returns.
    async fn up(db: &Database, steps: Option<u32>) -> Result<(), DbErr> {
        let migrations = Self::migrations();
        ensure_unique(&migrations)?;
        Self::install(db).await?;
        let applied = Self::get_applied_migrations(db).await?;
        report_issues(&find_issues(&migrations, &applied), Self::strict())?;
        let mut remaining = steps.map_or(usize::MAX, |s| s as usize);
        for migration in migrations {
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
            // Another migrator may have applied it since the list was read;
            // the write lock now held makes this check final.
            if is_applied(&txn, Self::migration_table_name(), migration.name()).await? {
                txn.rollback().await?;
                continue;
            }
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
    /// Unknown and out-of-order migrations are left alone: reverting does
    /// not depend on them, and failing here would block the way back from
    /// the state they describe.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Migration`] when two declared migrations share a
    /// name, before anything runs; [`DbErr::Driver`] when a statement or
    /// the transaction fails; any error the migration's `down` returns,
    /// including the default [`DbErr::Migration`] of an irreversible
    /// migration.
    async fn down(db: &Database, steps: Option<u32>) -> Result<(), DbErr> {
        let migrations = Self::migrations();
        ensure_unique(&migrations)?;
        Self::install(db).await?;
        let applied = Self::get_applied_migrations(db).await?;
        let mut remaining = steps.map_or(usize::MAX, |s| s as usize);
        for migration in migrations.into_iter().rev() {
            if remaining == 0 {
                break;
            }
            if !applied.iter().any(|a| a == migration.name()) {
                continue;
            }
            tracing::info!(name = migration.name(), "reverting migration");
            let txn = db.begin_with_mode(TransactionMode::Immediate).await?;
            // Another migrator may have reverted it since the list was read;
            // the write lock now held makes this check final.
            if !is_applied(&txn, Self::migration_table_name(), migration.name()).await? {
                txn.rollback().await?;
                continue;
            }
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
    /// Returns [`DbErr::Migration`] when two declared migrations share a
    /// name, before any table is dropped; [`DbErr::Driver`] when a query, a
    /// drop or the transaction fails; the errors of [`up`](Self::up).
    async fn fresh(db: &Database) -> Result<(), DbErr> {
        ensure_unique(&Self::migrations())?;
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
    /// Returns [`DbErr::Migration`] when [`strict`](Self::strict) is set
    /// and an applied migration is not declared, before anything is
    /// reverted; the errors of [`down`](Self::down) and [`up`](Self::up).
    async fn refresh(db: &Database) -> Result<(), DbErr> {
        // An unknown migration survives `down`, so a strict `up` would only
        // refuse it once everything had been reverted; check it first.
        if Self::strict() {
            let unknown: Vec<MigrationIssue> = Self::check(db)
                .await?
                .into_iter()
                .filter(|issue| matches!(issue, MigrationIssue::Unknown(_)))
                .collect();
            report_issues(&unknown, true)?;
        }
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

/// Whether `version` has a row in the bookkeeping table `table`.
///
/// Read inside the migration's transaction, so that the answer reflects
/// what other migrators committed before the write lock was taken.
///
/// # Errors
///
/// Returns [`DbErr::Driver`] when the query fails.
async fn is_applied<C: ConnectionTrait>(
    conn: &C,
    table: &'static str,
    version: &str,
) -> Result<bool, DbErr> {
    let stmt = Query::select()
        .column("version")
        .from(table)
        .and_where(Expr::col("version").eq(Expr::val(version)));
    Ok(conn
        .query_one(turso_orm::Build::to_statement(&stmt))
        .await?
        .is_some())
}

/// Checks that no two declared migrations share a name.
///
/// # Errors
///
/// Returns [`DbErr::Migration`] naming the first duplicate.
fn ensure_unique(migrations: &[Box<dyn MigrationTrait>]) -> Result<(), DbErr> {
    let mut seen = HashSet::new();
    match migrations.iter().find(|m| !seen.insert(m.name())) {
        Some(duplicate) => Err(DbErr::Migration(format!(
            "duplicate migration name `{}`",
            duplicate.name()
        ))),
        None => Ok(()),
    }
}

/// Compares the declared migrations with the applied ones.
///
/// A pending migration is out of order when any migration declared after
/// it is applied.
fn find_issues(migrations: &[Box<dyn MigrationTrait>], applied: &[String]) -> Vec<MigrationIssue> {
    let declared: HashSet<&str> = migrations.iter().map(|m| m.name()).collect();
    let is_applied = |name: &str| applied.iter().any(|a| a == name);
    let last_applied = migrations.iter().rposition(|m| is_applied(m.name()));
    let unknown = applied
        .iter()
        .filter(|a| !declared.contains(a.as_str()))
        .map(|a| MigrationIssue::Unknown(a.clone()));
    let out_of_order = migrations
        .iter()
        .take(last_applied.unwrap_or(0))
        .filter(|m| !is_applied(m.name()))
        .map(|m| MigrationIssue::OutOfOrder(m.name().to_owned()));
    unknown.chain(out_of_order).collect()
}

/// Logs each issue as a warning, or turns them into one error when `strict`.
///
/// # Errors
///
/// Returns [`DbErr::Migration`] listing every issue when `strict` is set
/// and there is at least one.
fn report_issues(issues: &[MigrationIssue], strict: bool) -> Result<(), DbErr> {
    if strict && !issues.is_empty() {
        let list: Vec<String> = issues.iter().map(ToString::to_string).collect();
        return Err(DbErr::Migration(list.join("; ")));
    }
    for issue in issues {
        tracing::warn!("{issue}");
    }
    Ok(())
}
