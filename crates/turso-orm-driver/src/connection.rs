//! The connection traits implemented by [`Database`] and [`Transaction`].
//!
//! Code above the driver should not care whether it runs on the pooled
//! handle or inside a transaction, so both implement the same three traits
//! and the entity layer is written against `impl ConnectionTrait`. The
//! traits are split by capability rather than merged into one so that a
//! type can offer execution without transactions, and so that the streaming
//! method — which cannot go through `async_trait` because its future borrows
//! the connection for the stream's lifetime — keeps its explicit signature.
//!
//! This module owns the trait definitions and their implementations for
//! [`Database`]; the [`Transaction`] implementations live next to the
//! transaction type. The actual work is delegated to `crate::executor`.
//!
//! - [`ConnectionTrait`]: execute statements and fetch rows;
//! - [`StreamTrait`]: stream rows lazily;
//! - [`TransactionTrait`]: start transactions, including the closure form.

use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;
use turso_sql::Statement;

use crate::database::Database;
use crate::error::Result;
use crate::executor::{self, Conn, ExecResult, Row, RowStream};
use crate::transaction::{Transaction, TransactionMode};

/// Executes statements and fetches rows.
///
/// # Errors
///
/// Every method returns [`Error::Turso`](crate::Error::Turso) when the
/// engine rejects the statement — with
/// [`ErrorKind::Busy`](crate::ErrorKind::Busy) under lock contention,
/// [`ErrorKind::Constraint`](crate::ErrorKind::Constraint) on a constraint
/// violation and [`ErrorKind::Other`](crate::ErrorKind::Other) otherwise.
/// The [`Database`] implementation additionally returns
/// [`Error::PoolTimeout`](crate::Error::PoolTimeout) when no connection is
/// free within the acquire timeout.
#[async_trait]
pub trait ConnectionTrait: Send + Sync {
    /// Runs a statement that returns no rows.
    async fn execute(&self, statement: Statement) -> Result<ExecResult>;

    /// Runs one or more `;`-separated statements without parameters.
    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult>;

    /// Fetches at most one row.
    async fn query_one(&self, statement: Statement) -> Result<Option<Row>>;

    /// Fetches all rows.
    async fn query_all(&self, statement: Statement) -> Result<Vec<Row>>;
}

/// Streams rows lazily.
pub trait StreamTrait: Send + Sync {
    /// Runs a query and streams its rows.
    ///
    /// # Errors
    ///
    /// The future fails with [`Error::Turso`](crate::Error::Turso) when the
    /// statement cannot be prepared or started, and the [`Database`]
    /// implementation with [`Error::PoolTimeout`](crate::Error::PoolTimeout)
    /// when no connection is free. Errors while stepping are yielded as
    /// items of the stream.
    fn stream<'a>(
        &'a self,
        statement: Statement,
    ) -> Pin<Box<dyn Future<Output = Result<RowStream<'a>>> + Send + 'a>>;
}

/// Starts transactions.
#[async_trait]
pub trait TransactionTrait: Send + Sync {
    /// Begins with `BEGIN DEFERRED`, or a savepoint when called on a
    /// transaction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Turso`](crate::Error::Turso) when the engine cannot
    /// start the transaction, and on [`Database`]
    /// [`Error::PoolTimeout`](crate::Error::PoolTimeout) when no connection
    /// is free within the acquire timeout.
    async fn begin(&self) -> Result<Transaction>;

    /// Begins with an explicit mode; the mode is ignored for nested
    /// transactions, which are always savepoints.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Turso`](crate::Error::Turso) when the engine cannot
    /// start the transaction — with
    /// [`ErrorKind::Busy`](crate::ErrorKind::Busy) when an `IMMEDIATE` or
    /// `EXCLUSIVE` lock cannot be taken within the busy timeout — and on
    /// [`Database`] [`Error::PoolTimeout`](crate::Error::PoolTimeout) when
    /// no connection is free.
    async fn begin_with_mode(&self, mode: TransactionMode) -> Result<Transaction>;

    /// Runs `callback` inside a transaction, committing on `Ok` and rolling
    /// back on `Err`.
    ///
    /// The callback receives `&Transaction` rather than owning it so that it
    /// cannot commit or roll back on its own and leave this method with a
    /// finished transaction.
    ///
    /// # Errors
    ///
    /// Returns the callback's error after rolling back, or the driver error
    /// converted through `E::from` when beginning, committing or rolling
    /// back fails.
    async fn transaction<F, T, E>(&self, callback: F) -> std::result::Result<T, E>
    where
        F: for<'c> FnOnce(
                &'c Transaction,
            )
                -> Pin<Box<dyn Future<Output = std::result::Result<T, E>> + Send + 'c>>
            + Send,
        T: Send,
        E: From<crate::Error> + Send,
    {
        let txn = self.begin().await?;
        match callback(&txn).await {
            Ok(value) => {
                txn.commit().await?;
                Ok(value)
            }
            Err(err) => {
                txn.rollback().await?;
                Err(err)
            }
        }
    }
}

#[async_trait]
impl ConnectionTrait for Database {
    async fn execute(&self, statement: Statement) -> Result<ExecResult> {
        let conn = self.acquire().await?;
        executor::execute(&conn, &statement).await
    }

    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult> {
        let conn = self.acquire().await?;
        executor::execute_unprepared(&conn, sql).await
    }

    async fn query_one(&self, statement: Statement) -> Result<Option<Row>> {
        let conn = self.acquire().await?;
        executor::query_one(&conn, &statement).await
    }

    async fn query_all(&self, statement: Statement) -> Result<Vec<Row>> {
        let conn = self.acquire().await?;
        executor::query_all(&conn, &statement).await
    }
}

impl StreamTrait for Database {
    fn stream<'a>(
        &'a self,
        statement: Statement,
    ) -> Pin<Box<dyn Future<Output = Result<RowStream<'a>>> + Send + 'a>> {
        Box::pin(async move {
            let conn = self.acquire().await?;
            // The stream must outlive this future, so it cannot borrow the
            // pooled guard. A clone of the engine connection is used to
            // run the query while the guard itself travels inside the
            // stream as the holder, returning the slot to the pool when the
            // stream is dropped.
            let raw: Conn = (*conn).clone();
            executor::stream(&raw, &statement, conn).await
        })
    }
}

#[async_trait]
impl TransactionTrait for Database {
    async fn begin(&self) -> Result<Transaction> {
        self.begin_with_mode(TransactionMode::Deferred).await
    }

    async fn begin_with_mode(&self, mode: TransactionMode) -> Result<Transaction> {
        let conn = self.acquire().await?;
        Transaction::begin_top(conn, mode).await
    }
}

/// Implements [`ConnectionTrait`] for `&T` by forwarding, so that a
/// borrowed handle satisfies `impl ConnectionTrait` bounds.
macro_rules! forward_ref {
    ($($t:ty),*) => {$(
        #[async_trait]
        impl ConnectionTrait for &$t {
            async fn execute(&self, statement: Statement) -> Result<ExecResult> {
                (**self).execute(statement).await
            }
            async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult> {
                (**self).execute_unprepared(sql).await
            }
            async fn query_one(&self, statement: Statement) -> Result<Option<Row>> {
                (**self).query_one(statement).await
            }
            async fn query_all(&self, statement: Statement) -> Result<Vec<Row>> {
                (**self).query_all(statement).await
            }
        }
    )*};
}
forward_ref!(Database, Transaction);
