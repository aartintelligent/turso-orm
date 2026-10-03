//! Transactions bound to one pooled connection, modeled by [`Transaction`].
//!
//! SQLite transactions are a property of the connection, so a transaction
//! must pin the pooled connection it started on and route every statement
//! through it; a second connection would see a different snapshot or block
//! on the first one's lock. Nesting is done with `SAVEPOINT`s on that same
//! connection, which is why a nested [`Transaction`] shares the parent's
//! state rather than acquiring anything.
//!
//! The engine keeps one stack of savepoints, so only the innermost open
//! transaction may act. A statement issued through a parent while a nested
//! transaction is open would run inside the nested savepoint and share its
//! fate; one issued after the top level finished would run outside any
//! transaction. Every handle therefore checks, before each statement, that
//! it is the top of the shared stack and that the top level is still open,
//! and fails with [`Error::Misuse`](crate::Error::Misuse) otherwise. Each
//! savepoint carries a unique identifier rather than its depth, so a stale
//! handle can never name a newer savepoint that took its place.
//!
//! `Drop` cannot await, so an unfinished transaction cannot roll itself
//! back synchronously. The module makes that safe in two ways: a dropped
//! top-level transaction discards its connection, which the engine rolls
//! back when the connection closes and which the pool never hands out
//! again; a dropped nested transaction records its position in the shared
//! stack and the parent runs `ROLLBACK TO SAVEPOINT` before its next
//! statement. The shallowest dropped position wins because rolling back to
//! it subsumes every deeper savepoint.
//!
//! This module owns the transaction lifecycle only. Statement execution is
//! delegated to `crate::executor` and the pool to `crate::database`.
//!
//! - [`Transaction`]: the handle, which must be committed or rolled back;
//! - [`TransactionMode`]: how a top-level transaction takes its locks.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use turso_sql::Statement;

use crate::connection::{ConnectionTrait, StreamTrait, TransactionTrait};
use crate::database::{PooledConnection, retry_busy};
use crate::error::{Error, Result};
use crate::executor::{self, Conn, ExecResult, Row, RowStream};

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

/// The savepoint bookkeeping shared by a transaction and its nested ones.
struct State {
    /// The identifiers of the open savepoints, outermost first; mirrors the
    /// engine's savepoint stack.
    savepoints: Vec<u64>,
    /// The identifier of the last savepoint created; incremented before use.
    last_id: u64,
    /// The stack position of the shallowest nested transaction dropped
    /// without commit or rollback; rolled back to before the next statement.
    pending_rollback: Option<usize>,
    /// Whether the top-level transaction has finished or been dropped.
    closed: bool,
}

/// The connection state shared by a transaction and its nested savepoints.
struct Shared {
    /// The pinned connection.
    conn: PooledConnection,
    /// The savepoint bookkeeping, never held across an `.await`.
    state: Mutex<State>,
}

impl Shared {
    /// Locks the savepoint bookkeeping.
    fn state(&self) -> MutexGuard<'_, State> {
        // Nothing panics while the lock is held, so a poisoned lock still
        // guards a consistent stack.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Applies the rollback owed by a dropped nested transaction, if any.
    ///
    /// Called before every statement and before finishing, so that work
    /// done inside a dropped savepoint never leaks into the parent.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Turso`] when the engine cannot roll back to or
    /// release the savepoint.
    async fn settle(&self) -> Result<()> {
        let pending = {
            let mut state = self.state();
            state
                .pending_rollback
                .take()
                .and_then(|index| Some((index, *state.savepoints.get(index)?)))
        };
        if let Some((index, id)) = pending {
            tracing::warn!(
                depth = index + 1,
                "rolling back nested transaction dropped without commit"
            );
            rollback_to(&self.conn, id).await?;
            self.state().savepoints.truncate(index);
        }
        Ok(())
    }
}

/// Rolls back to the savepoint `id` and releases it.
///
/// `ROLLBACK TO` alone leaves the savepoint on the stack, so it is released
/// afterwards to keep the engine's savepoint stack in step with the shared
/// one.
///
/// # Errors
///
/// Returns [`Error::Turso`] when either statement fails.
async fn rollback_to(conn: &Conn, id: u64) -> Result<()> {
    conn.execute_raw(&format!("ROLLBACK TO SAVEPOINT sp{id}"))
        .await?;
    conn.execute_raw(&format!("RELEASE SAVEPOINT sp{id}"))
        .await?;
    Ok(())
}

/// A transaction on one Turso connection.
///
/// Created with [`Database::begin`](crate::Database::begin) or
/// [`Database::begin_with_mode`](crate::Database::begin_with_mode). Nested
/// transactions (`txn.begin()`) are `SAVEPOINT`s on the same connection.
/// While a nested transaction is open it is the only one that may run
/// statements, begin, commit or roll back; its parents fail with
/// [`Error::Misuse`] until it is finished or dropped. Dropping an unfinished
/// transaction rolls it back: a top-level transaction discards its
/// connection, a nested one is rolled back to its savepoint before the
/// parent runs its next statement.
#[must_use = "a transaction must be committed or rolled back"]
pub struct Transaction {
    /// The connection state shared with the parent and children.
    shared: Arc<Shared>,
    /// This transaction's savepoint identifier; `None` for the top level.
    savepoint: Option<u64>,
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
    /// Returns [`Error::Turso`] when the engine cannot start the
    /// transaction — with [`ErrorKind::Busy`](crate::ErrorKind::Busy) once
    /// the retry budget is exhausted.
    pub(crate) async fn begin_top(conn: PooledConnection, mode: TransactionMode) -> Result<Self> {
        let budget = conn.options().busy_timeout.unwrap_or_default();
        retry_busy(budget, || async { conn.execute_raw(mode.sql()).await }).await?;
        Ok(Self {
            shared: Arc::new(Shared {
                conn,
                state: Mutex::new(State {
                    savepoints: Vec::new(),
                    last_id: 0,
                    pending_rollback: None,
                    closed: false,
                }),
            }),
            savepoint: None,
            depth: 0,
            open: true,
        })
    }

    /// Begins a nested transaction as a savepoint one level deeper.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`prepare`](Self::prepare);
    /// [`Error::Turso`] when the `SAVEPOINT` statement fails.
    async fn begin_nested(&self) -> Result<Self> {
        self.prepare().await?;
        let id = {
            let mut state = self.shared.state();
            state.last_id += 1;
            state.last_id
        };
        self.conn()
            .execute_raw(&format!("SAVEPOINT sp{id}"))
            .await?;
        self.shared.state().savepoints.push(id);
        Ok(Self {
            shared: Arc::clone(&self.shared),
            savepoint: Some(id),
            depth: self.depth + 1,
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

    /// Checks that this transaction may act, after applying the rollback
    /// owed by a dropped nested transaction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Misuse`] when the top-level transaction has
    /// finished, when a nested transaction is still open inside this one,
    /// or when this one was rolled back with a dropped parent;
    /// [`Error::Turso`] when the pending rollback fails.
    async fn prepare(&self) -> Result<()> {
        if self.shared.state().closed {
            return Err(Error::Misuse("transaction already finished".into()));
        }
        self.shared.settle().await?;
        let state = self.shared.state();
        let innermost = match self.savepoint {
            None => state.savepoints.is_empty(),
            Some(id) => state.savepoints.last() == Some(&id),
        };
        if innermost {
            return Ok(());
        }
        let alive = self
            .savepoint
            .is_none_or(|id| state.savepoints.contains(&id));
        Err(Error::Misuse(
            if alive {
                "a nested transaction is still open"
            } else {
                "nested transaction rolled back with its parent"
            }
            .into(),
        ))
    }

    /// Finishes the transaction, rolling back when `rollback` is set and
    /// committing otherwise.
    ///
    /// When the checks fail, the transaction is dropped unfinished and
    /// rolled back as `Drop` describes. Once they pass, `open` is cleared
    /// so that `Drop` stays inert even if the engine statement fails; a
    /// failed commit leaves the connection in an unknown state, which the
    /// pool detects through `is_autocommit`.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`prepare`](Self::prepare);
    /// [`Error::Turso`] when the finishing statement fails.
    async fn finish(mut self, rollback: bool) -> Result<()> {
        self.prepare().await?;
        self.open = false;
        match self.savepoint {
            None => {
                self.shared.state().closed = true;
                self.conn()
                    .execute_raw(if rollback { "ROLLBACK" } else { "COMMIT" })
                    .await?;
            }
            Some(id) => {
                if rollback {
                    rollback_to(self.conn(), id).await?;
                } else {
                    self.conn()
                        .execute_raw(&format!("RELEASE SAVEPOINT sp{id}"))
                        .await?;
                }
                self.shared.state().savepoints.pop();
            }
        }
        Ok(())
    }

    /// Commits the transaction — `COMMIT`, or `RELEASE SAVEPOINT` when
    /// nested.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Misuse`] when a nested transaction is still open or
    /// the transaction can no longer act, in which case it is rolled back;
    /// [`Error::Turso`] when the commit fails — an MVCC write conflict
    /// surfaces here with [`ErrorKind::Busy`](crate::ErrorKind::Busy).
    pub async fn commit(self) -> Result<()> {
        self.finish(false).await
    }

    /// Rolls the transaction back — `ROLLBACK`, or `ROLLBACK TO SAVEPOINT`
    /// when nested.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Misuse`] when a nested transaction is still open or
    /// the transaction can no longer act, in which case it is still rolled
    /// back; [`Error::Turso`] when the rollback fails.
    pub async fn rollback(self) -> Result<()> {
        self.finish(true).await
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        if !self.open {
            return;
        }
        let mut state = self.shared.state();
        let Some(id) = self.savepoint else {
            state.closed = true;
            drop(state);
            // Nothing can be awaited here, so the connection is withheld
            // from the pool; the engine rolls back when it is closed.
            tracing::warn!("transaction dropped without commit or rollback; discarding connection");
            self.shared.conn.discard();
            return;
        };
        // A savepoint already gone — under a finished top level, or rolled
        // back with a parent — owes nothing. Otherwise record the shallowest
        // dropped position: rolling back to it also undoes every deeper
        // savepoint.
        if state.closed {
            return;
        }
        if let Some(index) = state.savepoints.iter().position(|&s| s == id) {
            state.pending_rollback = Some(state.pending_rollback.map_or(index, |p| p.min(index)));
        }
    }
}

#[async_trait]
impl ConnectionTrait for Transaction {
    async fn execute(&self, statement: Statement) -> Result<ExecResult> {
        self.prepare().await?;
        executor::execute(self.conn(), &statement).await
    }

    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult> {
        self.prepare().await?;
        executor::execute_unprepared(self.conn(), sql).await
    }

    async fn query_one(&self, statement: Statement) -> Result<Option<Row>> {
        self.prepare().await?;
        executor::query_one(self.conn(), &statement).await
    }

    async fn query_all(&self, statement: Statement) -> Result<Vec<Row>> {
        self.prepare().await?;
        executor::query_all(self.conn(), &statement).await
    }
}

impl StreamTrait for Transaction {
    fn stream<'a>(
        &'a self,
        statement: Statement,
    ) -> Pin<Box<dyn Future<Output = Result<RowStream<'a>>> + Send + 'a>> {
        Box::pin(async move {
            self.prepare().await?;
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
