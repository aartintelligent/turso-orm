//! Execution layer: connection pool, transactions and typed rows for Turso.
//!
//! This crate is the execution layer of [turso-orm]. It wraps the [`turso`]
//! client in a small connection pool and gives the layers above a uniform
//! way to run a [`Statement`] — on the pooled handle or inside a
//! transaction — and to read the rows back by the Rust type they want. It
//! owns everything that touches a live connection: opening, pooling,
//! per-connection pragmas, transactions and savepoints, statement execution,
//! streaming, value decoding and error classification. It deliberately does
//! not own SQL generation, which is `turso-sql`'s job, nor entity mapping,
//! which is `turso-orm`'s.
//!
//! # Design
//!
//! - The pool hands out connections created with `db.connect()`, one per
//!   slot, and never multiplies a slot by cloning a `turso::Connection`:
//!   clones share one engine connection and would serialise on it. A
//!   connection goes back to the idle list only when it is in autocommit
//!   mode, so a transaction that was dropped mid-way can never leak into
//!   the next borrower.
//! - A [`Transaction`] pins one pooled connection for its whole life.
//!   Nested transactions are `SAVEPOINT`s on that same connection. Because
//!   `Drop` cannot await, rolling back a dropped transaction is deferred: a
//!   top-level one discards its connection, a nested one records its depth
//!   and the rollback runs before the parent's next statement.
//! - Rows keep their storage class and are decoded on access through
//!   [`FromValue`], with SQLite-style leniency — integers become booleans,
//!   text parses into dates and UUIDs — so the same column can be read as
//!   whatever the caller asks for.
//! - `turso::Error` carries only strings for most variants, so
//!   [`ErrorKind`] is derived by variant and, for MVCC conflicts, by message.
//!
//! # Example
//!
//! ```no_run
//! use turso_orm_driver::{ConnectOptions, ConnectionTrait, Database};
//! use turso_sql::Statement;
//!
//! # async fn boot() -> Result<(), turso_orm_driver::Error> {
//! let db = Database::connect(ConnectOptions::new("app.db")).await?;
//! db.execute_unprepared("CREATE TABLE IF NOT EXISTS t (id INTEGER PRIMARY KEY, n TEXT)").await?;
//! let row = db.query_one(Statement::from_string("SELECT COUNT(*) AS n FROM t")).await?;
//! let count: i64 = row.expect("one row").get("n")?;
//! # Ok(())
//! # }
//! ```
//!
//! [turso-orm]: https://github.com/aartintelligent/turso-orm
#![cfg_attr(docsrs, feature(doc_cfg))]

mod connection;
mod database;
mod decode;
mod error;
mod executor;
mod options;
mod transaction;

pub use connection::{ConnectionTrait, StreamTrait, TransactionTrait};
pub use database::Database;
pub use decode::FromValue;
pub use error::{ConstraintKind, Error, ErrorKind, Result};
pub use executor::{ExecResult, Row, RowStream};
#[cfg(feature = "serverless")]
pub use options::RemoteOptions;
#[cfg(feature = "sync")]
pub use options::SyncOptions;
pub use options::{ConnectOptions, Encryption, Experimental, Source};
pub use transaction::{Transaction, TransactionMode};

/// Re-export of the underlying Turso client, so callers can reach engine
/// types such as `turso::Value` without depending on the crate themselves.
pub use turso;
/// Re-export of the SQL layer, so a single dependency gives the whole
/// query language.
pub use turso_sql;
pub use turso_sql::{Statement, Value};
