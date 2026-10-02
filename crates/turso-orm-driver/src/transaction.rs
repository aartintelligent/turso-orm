//! Transactions bound to one pooled connection, modeled by [`Transaction`].
//!
//! SQLite transactions are a property of the connection, so a transaction
//! must pin the pooled connection it started on and route every statement
//! through it; a second connection would see a different snapshot or block
//! on the first one's lock. Nesting is done with `SAVEPOINT`s on that same
//! connection, which is why a nested [`Transaction`] shares the parent's
//! state rather than acquiring anything.
//!
//! `Drop` cannot await, so an unfinished transaction cannot roll itself
//! back synchronously. The module makes that safe in two ways: a dropped
//! top-level transaction discards its connection, which the engine rolls
//! back when the connection closes and which the pool never hands out
//! again; a dropped nested transaction records its depth in the shared
//! state and the parent runs `ROLLBACK TO SAVEPOINT` before its next
//! statement. The shallowest dropped depth wins because rolling back to it
//! subsumes every deeper savepoint.
//!
//! This module owns the transaction lifecycle only. Statement execution is
//! delegated to `crate::executor` and the pool to `crate::database`.
//!
//! - [`Transaction`]: the handle, which must be committed or rolled back;
//! - [`TransactionMode`]: how a top-level transaction takes its locks.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use async_trait::async_trait;
use turso_sql::Statement;

use crate::connection::{ConnectionTrait, StreamTrait, TransactionTrait};
use crate::database::{PooledConnection, retry_busy};
use crate::error::Result;
use crate::executor::{self, Conn, ExecResult, Row, RowStream};

/// The sentinel stored in `Shared::pending_rollback` when no nested
/// transaction was dropped. `u32::MAX` so that `fetch_min` with any real
/// depth replaces it.
const NO_PENDING: u32 = u32::MAX;

/// How a top-level transaction is started.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransactionMode {
    /// `BEGIN DEFERRED`, the default: locks are taken by the first
    /// statement.
    #[default]
    Deferred,
    /// `BEGIN IMMEDIATE`: the write lock is taken now, so a writer finds out
    /// about contention before doing any work.
    Immediate,
    /// `BEGIN EXCLUSIVE`: the exclusive lock is taken now.
    Exclusive,
    /// `BEGIN CONCURRENT`: an optimistic MVCC transaction whose conflicts
    /// surface at commit. Requires
    /// [`ConnectOptions::mvcc`](crate::ConnectOptions::mvcc).
    Concurrent,
}

impl TransactionMode {
    /// The `BEGIN` statement for this mode.
    fn sql(self) -> &'static str {
        match self {
            Self::Deferred => "BEGIN DEFERRED",
            Self::Immediate => "BEGIN IMMEDIATE",
            Self::Exclusive => "BEGIN EXCLUSIVE",
            Self::Concurrent => "BEGIN CONCURRENT",
        }
    }
}

/// The connection state shared by a transaction and its nested savepoints.
struct Shared {
    /// The pinned connection.
    conn: PooledConnection,
    /// The depth of the deepest savepoint still open.
    depth: AtomicU32,
    /// The depth of the shallowest nested transaction dropped without
    /// commit or rollback, or [`NO_PENDING`]; rolled back to before the
    /// next statement.
    pending_rollback: AtomicU32,
}

impl Shared {
    /// Applies the rollback owed by a dropped nested transaction, if any.
    ///
    /// Called before every statement and before finishing, so that work
    /// done inside a dropped savepoint never leaks into the parent.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Turso`](crate::Error::Turso) when the engine cannot
    /// roll back to or release the savepoint.
    async fn settle(&self) -> Result<()> {
        let pending = self.pending_rollback.swap(NO_PENDING, Ordering::AcqRel);
        if pending != NO_PENDING {
            tracing::warn!(
                depth = pending,
                "rolling back nested transaction dropped without commit"
            );
            rollback_to(&self.conn, pending).await?;
            self.depth.store(pending - 1, Ordering::Release);
        }
        Ok(())
    }
}

/// Rolls back to the savepoint at `depth` and releases it.
///
/// `ROLLBACK TO` alone leaves the savepoint on the stack, so it is released
/// afterwards to keep the engine's savepoint stack in step with `depth`.
///
/// # Errors
///
/// Returns [`Error::Turso`](crate::Error::Turso) when either statement
/// fails.
async fn rollback_to(conn: &Conn, depth: u32) -> Result<()> {
    conn.execute_raw(&format!("ROLLBACK TO SAVEPOINT sp{depth}"))
        .await?;
    conn.execute_raw(&format!("RELEASE SAVEPOINT sp{depth}"))
        .await?;
    Ok(())
}

/// A transaction on one Turso connection.
///
/// Created with [`Database::begin`](crate::Database::begin) or
/// [`Database::begin_with_mode`](crate::Database::begin_with_mode). Nested
/// transactions (`txn.begin()`) are `SAVEPOINT`s on the same connection.
/// Dropping an unfinished transaction rolls it back: a top-level transaction
/// discards its connection, a nested one is rolled back to its savepoint
/// before the parent runs its next statement.
#[must_use = "a transaction must be committed or rolled back"]
pub struct Transaction {
    /// The connection state shared with the parent and children.
    shared: Arc<Shared>,
    /// This transaction's savepoint depth; `0` for the top level.
    depth: u32,
    /// Whether the transaction is still unfinished; cleared by `finish` so
    /// that `Drop` does nothing after a commit or rollback.
    open: bool,
}

impl fmt::Debug for Transaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transaction")
            .field("depth", &self.depth)
            .field("open", &self.open)
            .finish_non_exhaustive()
    }
}

impl Transaction {
    /// Begins a top-level transaction on `conn` with the given mode.
    ///
    /// `BEGIN IMMEDIATE` and `BEGIN EXCLUSIVE` fail immediately when another
    /// writer holds the lock, so the statement is retried for the busy
    /// timeout; a `None` busy timeout gives a zero budget and no retry.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Turso`](crate::Error::Turso) when the engine cannot
    /// start the transaction — with
    /// [`ErrorKind::Busy`](crate::ErrorKind::Busy) once the retry budget is
    /// exhausted.
    pub(crate) async fn begin_top(conn: PooledConnection, mode: TransactionMode) -> Result<Self> {
        let budget = conn.options().busy_timeout.unwrap_or_default();
        retry_busy(budget, || async { conn.execute_raw(mode.sql()).await }).await?;
        Ok(Self {
            shared: Arc::new(Shared {
                conn,
                depth: AtomicU32::new(0),
                pending_rollback: AtomicU32::new(NO_PENDING),
            }),
            depth: 0,
            open: true,
        })
    }

    /// Begins a nested transaction as a savepoint one level deeper.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Turso`](crate::Error::Turso) when a pending
    /// rollback or the `SAVEPOINT` statement fails.
    async fn begin_nested(&self) -> Result<Self> {
        self.shared.settle().await?;
        let depth = self.shared.depth.load(Ordering::Acquire) + 1;
        self.shared
            .conn
            .execute_raw(&format!("SAVEPOINT sp{depth}"))
            .await?;
        self.shared.depth.store(depth, Ordering::Release);
        Ok(Self {
            shared: Arc::clone(&self.shared),
            depth,
            open: true,
        })
    }

    /// The savepoint depth: `0` for a top-level transaction.
    pub fn depth(&self) -> u32 {
        self.depth
    }

    /// The pinned engine connection.
    fn conn(&self) -> &Conn {
        &self.shared.conn
    }

    /// Finishes the transaction, rolling back when `rollback` is set and
    /// committing otherwise.
    ///
    /// `open` is cleared first so that `Drop` stays inert even if the
    /// engine statement fails; a failed commit leaves the connection in an
    /// unknown state, which the pool detects through `is_autocommit`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Turso`](crate::Error::Turso) when a pending rollback
    /// or the finishing statement fails.
    async fn finish(mut self, rollback: bool) -> Result<()> {
        self.open = false;
        self.shared.settle().await?;
        if self.depth == 0 {
            self.conn()
                .execute_raw(if rollback { "ROLLBACK" } else { "COMMIT" })
                .await?;
        } else if rollback {
            rollback_to(self.conn(), self.depth).await?;
            self.shared.depth.store(self.depth - 1, Ordering::Release);
        } else {
            self.conn()
                .execute_raw(&format!("RELEASE SAVEPOINT sp{}", self.depth))
                .await?;
            self.shared.depth.store(self.depth - 1, Ordering::Release);
        }
        Ok(())
    }

    /// Commits the transaction — `COMMIT`, or `RELEASE SAVEPOINT` when
    /// nested.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Turso`](crate::Error::Turso) when the commit fails;
    /// an MVCC write conflict surfaces here with
    /// [`ErrorKind::Busy`](crate::ErrorKind::Busy).
    pub async fn commit(self) -> Result<()> {
        self.finish(false).await
    }

    /// Rolls the transaction back — `ROLLBACK`, or `ROLLBACK TO SAVEPOINT`
    /// when nested.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Turso`](crate::Error::Turso) when the rollback
    /// fails.
    pub async fn rollback(self) -> Result<()> {
        self.finish(true).await
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        if !self.open {
            return;
        }
        if self.depth == 0 {
            // Nothing can be awaited here, so the connection is withheld
            // from the pool; the engine rolls back when it is closed.
            tracing::warn!("transaction dropped without commit or rollback; discarding connection");
            self.shared.conn.discard();
        } else {
            // Record the shallowest dropped depth: rolling back to it also
            // undoes every deeper savepoint, so a deeper pending depth is
            // superseded and a shallower one is kept.
            self.shared
                .pending_rollback
                .fetch_min(self.depth, Ordering::AcqRel);
        }
    }
}

#[async_trait]
impl ConnectionTrait for Transaction {
    async fn execute(&self, statement: Statement) -> Result<ExecResult> {
        self.shared.settle().await?;
        executor::execute(self.conn(), &statement).await
    }

    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult> {
        self.shared.settle().await?;
        executor::execute_unprepared(self.conn(), sql).await
    }

    async fn query_one(&self, statement: Statement) -> Result<Option<Row>> {
        self.shared.settle().await?;
        executor::query_one(self.conn(), &statement).await
    }

    async fn query_all(&self, statement: Statement) -> Result<Vec<Row>> {
        self.shared.settle().await?;
        executor::query_all(self.conn(), &statement).await
    }
}

impl StreamTrait for Transaction {
    fn stream<'a>(
        &'a self,
        statement: Statement,
    ) -> Pin<Box<dyn Future<Output = Result<RowStream<'a>>> + Send + 'a>> {
        Box::pin(async move {
            self.shared.settle().await?;
            // The stream borrows the transaction, which already pins the
            // connection, so there is nothing extra to hold.
            executor::stream(self.conn(), &statement, ()).await
        })
    }
}

#[async_trait]
impl TransactionTrait for Transaction {
    async fn begin(&self) -> Result<Transaction> {
        self.begin_nested().await
    }

    async fn begin_with_mode(&self, _mode: TransactionMode) -> Result<Transaction> {
        // Nested transactions are savepoints, which have no mode; the mode
        // only applies at depth 0.
        self.begin_nested().await
    }
}
