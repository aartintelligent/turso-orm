//! The pooled database handle, modeled by [`Database`].
//!
//! Turso connections are cheap to open but not free, and the engine
//! serialises work on a single connection, so the handle keeps a bounded
//! pool of them. The same pool serves the serverless client, where a
//! connection is a server-side session that carries transaction state
//! between HTTP requests. The pool is a semaphore for the slot count plus an idle
//! list: a borrower takes a permit, pops an idle connection or opens a new
//! one with `db.connect()`, and the connection returns to the idle list on
//! drop. Slots are never multiplied by cloning a connection,
//! because clones share one engine connection and would serialise on it.
//!
//! A connection goes back to the idle list only when it is in autocommit
//! mode. Anything else — a transaction dropped without commit or rollback,
//! a statement left mid-way — is a state the next borrower must not
//! inherit, so the connection is dropped instead and the engine rolls back
//! whatever it held; a fresh one is opened on the next acquire.
//!
//! This module owns the engine handle, the pool and the per-connection
//! setup. Statement execution lives in `crate::executor`, the connection
//! traits in `crate::connection` and transactions in `crate::transaction`.
//!
//! - [`Database`]: the cloneable handle;
//! - [`PooledConnection`]: a checked-out connection that returns itself on
//!   drop;
//! - [`retry_busy`]: the backoff loop used where a busy error is expected.

use std::fmt;
use std::future::Future;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::error::{Error, Result};
use crate::executor::Conn;
use crate::options::{ConnectOptions, Source};
use turso_sql::Statement;

/// The engine handle behind the pool.
enum Engine {
    /// A local file or in-memory database.
    Local(turso::Database),
    /// An embedded replica synchronised with Turso Cloud.
    #[cfg(feature = "sync")]
    Sync(turso::sync::Database),
    /// A Turso Cloud database reached over HTTP.
    #[cfg(feature = "serverless")]
    Remote(turso_serverless::Database),
}

/// The state shared by every clone of a [`Database`].
pub(crate) struct Inner {
    /// The engine handle.
    engine: Engine,
    /// The options the database was opened with.
    pub(crate) options: ConnectOptions,
    /// Connections that are open and not checked out.
    idle: Mutex<Vec<Conn>>,
    /// One permit per pool slot, whether the slot's connection is idle or
    /// not yet opened.
    permits: Arc<Semaphore>,
}

/// A Turso database with a pool of connections.
///
/// Cloning is cheap; every clone shares the same engine handle and pool.
#[derive(Clone)]
pub struct Database {
    /// The shared state.
    pub(crate) inner: Arc<Inner>,
}

impl fmt::Debug for Database {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Database")
            .field("source", &self.inner.options.source)
            .field("max_connections", &self.inner.options.max_connections)
            .finish_non_exhaustive()
    }
}

impl Database {
    /// Opens the database described by `options`.
    ///
    /// One connection is opened eagerly and dropped back into the pool so
    /// that a bad path, a wrong key or a rejected pragma fails here rather
    /// than on the first query.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidOptions`] when encryption is combined with a
    /// sync source, and [`Error::Turso`] with [`ErrorKind::Connection`] or
    /// [`ErrorKind::Other`] when the engine cannot open the database, open
    /// the first connection or apply its pragmas.
    ///
    /// [`ErrorKind::Connection`]: crate::ErrorKind::Connection
    /// [`ErrorKind::Other`]: crate::ErrorKind::Other
    pub async fn connect(options: impl Into<ConnectOptions>) -> Result<Self> {
        let options = options.into();
        let engine = match &options.source {
            Source::Memory => Engine::Local(options.local_builder(":memory:").build().await?),
            Source::File(path) => Engine::Local(
                options
                    .local_builder(&path.to_string_lossy())
                    .build()
                    .await?,
            ),
            #[cfg(feature = "sync")]
            Source::Sync(sync) => {
                // The sync builder has no encryption hook, so the option
                // would be silently ignored; refuse it instead.
                if options.encryption.is_some() {
                    return Err(Error::InvalidOptions(
                        "local encryption is not supported together with sync".into(),
                    ));
                }
                let mut builder = turso::sync::Builder::new_remote(&sync.path.to_string_lossy())
                    .with_remote_url(&sync.remote_url)
                    .bootstrap_if_empty(sync.bootstrap_if_empty);
                if let Some(token) = &sync.auth_token {
                    builder = builder.with_auth_token(token);
                }
                Engine::Sync(builder.build().await?)
            }
            #[cfg(feature = "serverless")]
            Source::Remote(remote) => {
                if options.encryption.is_some() {
                    return Err(Error::InvalidOptions(
                        "local encryption does not apply to a remote database".into(),
                    ));
                }
                let mut builder = turso_serverless::Builder::new_remote(remote.url.clone());
                if let Some(token) = &remote.auth_token {
                    builder = builder.with_auth_token(token.clone());
                }
                if let Some(key) = &remote.remote_encryption_key {
                    builder = builder.with_remote_encryption_key(key.clone());
                }
                Engine::Remote(builder.build().await?)
            }
        };
        let db = Self {
            inner: Arc::new(Inner {
                engine,
                permits: Arc::new(Semaphore::new(options.max_connections)),
                idle: Mutex::new(Vec::with_capacity(options.max_connections)),
                options,
            }),
        };
        // Warm one connection so that misconfiguration fails fast; dropping
        // it parks it in the idle list for the first real borrower.
        drop(db.acquire().await?);
        Ok(db)
    }

    /// The options this database was opened with.
    pub fn options(&self) -> &ConnectOptions {
        &self.inner.options
    }

    /// Checks that the database answers queries.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PoolTimeout`] when no connection is free within the
    /// acquire timeout, [`Error::Misuse`] when the pool is closed, and
    /// [`Error::Turso`] when a connection cannot be opened or the probe
    /// query fails.
    pub async fn ping(&self) -> Result<()> {
        let conn = self.acquire().await?;
        crate::executor::query_one(&conn, &Statement::from_string("SELECT 1")).await?;
        Ok(())
    }

    /// Pushes local changes to Turso Cloud — embedded replica only.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidOptions`] when this database is not a
    /// replica, and [`Error::Turso`] when the sync fails.
    #[cfg(feature = "sync")]
    #[cfg_attr(docsrs, doc(cfg(feature = "sync")))]
    pub async fn push(&self) -> Result<()> {
        match &self.inner.engine {
            Engine::Sync(db) => Ok(db.push().await?),
            _ => Err(Error::InvalidOptions("not an embedded replica".into())),
        }
    }

    /// Pulls remote changes from Turso Cloud — embedded replica only — and
    /// returns whether anything changed.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidOptions`] when this database is not a
    /// replica, and [`Error::Turso`] when the sync fails.
    #[cfg(feature = "sync")]
    #[cfg_attr(docsrs, doc(cfg(feature = "sync")))]
    pub async fn pull(&self) -> Result<bool> {
        match &self.inner.engine {
            Engine::Sync(db) => Ok(db.pull().await?),
            _ => Err(Error::InvalidOptions("not an embedded replica".into())),
        }
    }

    /// Takes a connection from the pool, opening a new one if none is idle.
    ///
    /// The permit is held by the returned guard, so the pool never has more
    /// than `max_connections` connections checked out or idle.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PoolTimeout`] when no permit is free within the
    /// acquire timeout, [`Error::Misuse`] when the semaphore is closed or
    /// the idle-list mutex is poisoned, and [`Error::Turso`] when a new
    /// connection cannot be opened or configured.
    pub(crate) async fn acquire(&self) -> Result<PooledConnection> {
        let permit = tokio::time::timeout(
            self.inner.options.acquire_timeout,
            Arc::clone(&self.inner.permits).acquire_owned(),
        )
        .await
        .map_err(|_| Error::PoolTimeout)?
        .map_err(|_| Error::Misuse("connection pool closed".into()))?;

        // The lock is released before the await below so that opening a
        // connection never blocks other borrowers from popping idle ones.
        let idle = self
            .inner
            .idle
            .lock()
            .map_err(|_| Error::Misuse("pool mutex poisoned".into()))?
            .pop();
        let conn = match idle {
            Some(conn) => conn,
            None => self.open_connection().await?,
        };
        Ok(PooledConnection {
            conn: Some(conn),
            pool: Arc::clone(&self.inner),
            _permit: permit,
            discard: AtomicBool::new(false),
        })
    }

    /// Opens a new engine connection and applies the per-connection
    /// settings.
    ///
    /// Pragmas are per connection in SQLite, so every connection the pool
    /// opens must be configured the same way or borrowers would observe
    /// different behaviour depending on which slot they get.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Turso`] when the engine cannot open the connection
    /// or rejects one of the settings.
    async fn open_connection(&self) -> Result<Conn> {
        let conn = match &self.inner.engine {
            Engine::Local(db) => Conn::Embedded(db.connect()?),
            #[cfg(feature = "sync")]
            Engine::Sync(db) => Conn::Embedded(db.connect().await?),
            #[cfg(feature = "serverless")]
            Engine::Remote(db) => Conn::Remote(db.connect()?),
        };
        let options = &self.inner.options;
        if let Some(timeout) = options.busy_timeout {
            conn.busy_timeout(timeout)?;
        }
        if options.foreign_keys {
            conn.pragma_update("foreign_keys", "ON").await?;
        }
        // MVCC is a property of the local engine; a remote session has no
        // journal to switch.
        if options.mvcc && matches!(conn, Conn::Embedded(_)) {
            conn.pragma_update("journal_mode", "'mvcc'").await?;
        }
        for (name, value) in &options.pragmas {
            conn.pragma_update(name, value).await?;
        }
        Ok(conn)
    }
}

/// A connection checked out of the pool.
///
/// The connection returns to the idle list on drop unless it was discarded
/// or is no longer in autocommit mode; the permit is released either way.
pub(crate) struct PooledConnection {
    /// The connection; `None` only once `drop` has taken it.
    conn: Option<Conn>,
    /// The pool to return the connection to.
    pool: Arc<Inner>,
    /// The slot permit, released when the guard is dropped.
    _permit: OwnedSemaphorePermit,
    /// Whether the connection must be dropped instead of returned.
    discard: AtomicBool,
}

impl PooledConnection {
    /// Marks the connection so that it is not returned to the pool.
    ///
    /// Used when the connection's state is unknown, for example because a
    /// transaction was dropped without commit or rollback.
    pub(crate) fn discard(&self) {
        self.discard.store(true, Ordering::Release);
    }

    /// The options of the pool this connection belongs to.
    pub(crate) fn options(&self) -> &ConnectOptions {
        &self.pool.options
    }
}

impl Deref for PooledConnection {
    type Target = Conn;

    /// The underlying engine connection.
    ///
    /// # Panics
    ///
    /// When called after `drop` has taken the connection, which safe code
    /// cannot do since the field is only emptied inside `Drop`.
    fn deref(&self) -> &Self::Target {
        self.conn.as_ref().expect("connection present until drop")
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        // A connection that is not in autocommit mode still holds a
        // transaction — typically one that was dropped without commit or
        // rollback — and must not be handed to the next borrower; dropping
        // it makes the engine roll back. An error from `is_autocommit` is
        // treated the same way, since the connection's state is unknown.
        if let Some(conn) = self.conn.take()
            && !self.discard.load(Ordering::Acquire)
            && conn.is_autocommit().unwrap_or(false)
            && let Ok(mut idle) = self.pool.idle.lock()
        {
            idle.push(conn);
        }
    }
}

impl fmt::Debug for PooledConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PooledConnection").finish_non_exhaustive()
    }
}

/// Retries `op` with exponential backoff while Turso reports lock
/// contention, for at most `budget`.
///
/// The engine's own busy handler only covers lock waits inside a statement;
/// `BEGIN IMMEDIATE` under contention and MVCC commit conflicts surface as
/// busy errors immediately, so callers that expect them wrap the call in
/// this loop.
///
/// # Errors
///
/// Returns whatever `op` returns once it fails with a non-busy error or the
/// budget is exhausted, so [`Error::Turso`] with
/// [`ErrorKind::Busy`](crate::ErrorKind::Busy) is what a persistent lock
/// produces.
pub(crate) async fn retry_busy<T, F, Fut>(budget: Duration, mut op: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let start = std::time::Instant::now();
    // The delay starts small enough not to add latency to a lock that is
    // about to clear and doubles up to a cap that keeps the loop responsive
    // once the budget runs into seconds.
    let mut delay = Duration::from_millis(5);
    loop {
        match op().await {
            Err(err) if err.is_busy() && start.elapsed() < budget => {
                tracing::debug!(%err, ?delay, "busy, retrying");
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(Duration::from_millis(250));
            }
            other => return other,
        }
    }
}
