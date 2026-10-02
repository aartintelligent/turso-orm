//! The schema manager handed to migrations, modeled by [`SchemaManager`].
//!
//! The manager is a thin view over the transaction a migration runs in. It
//! exists so that `up` and `down` receive one argument that can both run
//! DDL and inspect the catalog, while keeping the transaction itself
//! reachable through [`SchemaManager::get_connection`] for data migrations.
//! It never commits or rolls back; the migrator owns the transaction's
//! lifetime.
//!
//! Catalog lookups go through `sqlite_schema` and `pragma_table_info`
//! because they are the only portable way to ask SQLite what exists; the
//! same `has_table` helper serves the migrator's own check for the
//! bookkeeping table.

use turso_orm::sql::{
    AlterTable, Build, CreateIndex, CreateTable, DropIndex, DropTable, Statement,
};
use turso_orm::{ConnectionTrait, DbErr, Transaction};

/// Runs DDL inside the migration's transaction and inspects the catalog.
#[derive(Debug)]
pub struct SchemaManager<'c> {
    /// The transaction the migration runs in.
    conn: &'c Transaction,
}

impl<'c> SchemaManager<'c> {
    /// Wraps a transaction.
    pub fn new(conn: &'c Transaction) -> Self {
        Self { conn }
    }

    /// The transaction the migration runs in, for data migrations and raw statements.
    pub fn get_connection(&self) -> &'c Transaction {
        self.conn
    }

    /// Executes any DDL or DML statement builder.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the statement fails.
    pub async fn exec_stmt(&self, stmt: impl Build) -> Result<(), DbErr> {
        self.conn.execute(stmt.to_statement()).await?;
        Ok(())
    }

    /// Executes a `CREATE TABLE`.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the statement fails.
    pub async fn create_table(&self, stmt: CreateTable) -> Result<(), DbErr> {
        self.exec_stmt(stmt).await
    }

    /// Executes an `ALTER TABLE`.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the statement fails.
    pub async fn alter_table(&self, stmt: AlterTable) -> Result<(), DbErr> {
        self.exec_stmt(stmt).await
    }

    /// Executes a `DROP TABLE`.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the statement fails.
    pub async fn drop_table(&self, stmt: DropTable) -> Result<(), DbErr> {
        self.exec_stmt(stmt).await
    }

    /// Executes a `CREATE INDEX`.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the statement fails.
    pub async fn create_index(&self, stmt: CreateIndex) -> Result<(), DbErr> {
        self.exec_stmt(stmt).await
    }

    /// Executes a `DROP INDEX`.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the statement fails.
    pub async fn drop_index(&self, stmt: DropIndex) -> Result<(), DbErr> {
        self.exec_stmt(stmt).await
    }

    /// Whether a table named `table` exists.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Migration`] when the catalog query returns no row;
    /// [`DbErr::Driver`] when the query fails.
    pub async fn has_table(&self, table: &str) -> Result<bool, DbErr> {
        has_table(self.conn, table).await
    }

    /// Whether `table` has a column named `column`.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Migration`] when the catalog query returns no row;
    /// [`DbErr::Driver`] when the query fails.
    pub async fn has_column(&self, table: &str, column: &str) -> Result<bool, DbErr> {
        count_positive(
            self.conn,
            Statement::from_sql_and_values(
                "SELECT COUNT(*) AS n FROM pragma_table_info(?) WHERE name = ?",
                [table, column],
            ),
        )
        .await
    }

    /// Whether an index named `index` exists.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Migration`] when the catalog query returns no row;
    /// [`DbErr::Driver`] when the query fails.
    pub async fn has_index(&self, index: &str) -> Result<bool, DbErr> {
        count_positive(
            self.conn,
            Statement::from_sql_and_values(
                "SELECT COUNT(*) AS n FROM sqlite_schema WHERE type = 'index' AND name = ?",
                [index],
            ),
        )
        .await
    }
}

/// Whether a table named `table` exists, on any connection.
///
/// Shared with the migrator, which needs the check before any transaction
/// is open.
///
/// # Errors
///
/// Returns [`DbErr::Migration`] when the catalog query returns no row;
/// [`DbErr::Driver`] when the query fails.
pub(crate) async fn has_table<C: ConnectionTrait>(conn: &C, table: &str) -> Result<bool, DbErr> {
    count_positive(
        conn,
        Statement::from_sql_and_values(
            "SELECT COUNT(*) AS n FROM sqlite_schema WHERE type = 'table' AND name = ?",
            [table],
        ),
    )
    .await
}

/// Runs a `SELECT COUNT(*) AS n ...` statement and reports whether the count is positive.
///
/// # Errors
///
/// Returns [`DbErr::Migration`] when the query returns no row, which a
/// `COUNT(*)` never should; [`DbErr::Driver`] when the query fails or `n`
/// cannot be decoded.
async fn count_positive<C: ConnectionTrait>(conn: &C, stmt: Statement) -> Result<bool, DbErr> {
    let row = conn
        .query_one(stmt)
        .await?
        .ok_or_else(|| DbErr::Migration("catalog query returned no row".into()))?;
    Ok(row.get::<i64>("n")? > 0)
}
