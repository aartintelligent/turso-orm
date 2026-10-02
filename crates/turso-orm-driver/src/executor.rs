//! Statement execution on a raw engine connection, shared by the pool and transactions.
//!
//! Both [`Database`] and [`Transaction`] end up with a [`Conn`] and a
//! [`Statement`]; everything from there — preparing through the engine's
//! statement cache, binding the values, collecting or streaming rows and
//! reading the change counters — is identical and lives here once. The
//! module owns the [`Row`] type as well, because the column index a row
//! carries is built from the engine's result set at execution time.
//!
//! Two engines can sit behind a [`Conn`]: the embedded `turso` client, for
//! in-memory and file databases and embedded replicas, and the
//! `turso_serverless` HTTP client for Turso Cloud, behind the `serverless`
//! feature. The two crates expose the same method names on purpose, so the
//! engine-specific primitives are written once as a macro and instantiated
//! per crate; [`Conn`] dispatches to the right instance. Rows carry
//! `turso_sql::Value` rather than either engine's value type, which is what
//! keeps decoding independent of the engine.
//!
//! Statements are prepared with `prepare_cached`, so the same SQL text
//! reuses a compiled statement on the same connection; that is why the SQL
//! layer binds paging values as parameters instead of inlining them. A
//! cached statement is shared, so a query that stops early drains the
//! remaining rows rather than leaving a cursor open on it.
//!
//! Decoding is not done here: a [`Row`] keeps the storage values and
//! [`FromValue`] converts on access, so the same column can be read as
//! different Rust types.
//!
//! - [`Conn`]: the engine connection behind the pool;
//! - [`Row`] and [`ExecResult`]: what callers get back;
//! - [`RowStream`]: the boxed stream type of [`StreamTrait`];
//! - the `pub(crate)` functions: the execution primitives.
//!
//! [`Database`]: crate::Database
//! [`Transaction`]: crate::Transaction
//! [`StreamTrait`]: crate::StreamTrait

use std::collections::HashMap;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures_util::Stream;
use turso_sql::{Statement, Value};

use crate::decode::FromValue;
use crate::error::{Error, Result};

/// The result of a statement that does not return rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExecResult {
    /// The value of `last_insert_rowid()` after the statement.
    pub last_insert_id: i64,
    /// The number of rows changed.
    pub rows_affected: u64,
}

/// A result row.
///
/// Values are kept in their storage class and decoded on access with
/// [`FromValue`], so the same column can be read as `i64`, `bool` or
/// `String`. The column index is shared between all rows of one result set.
#[derive(Clone)]
pub struct Row {
    /// The column names and lookup index, shared across the result set.
    columns: Arc<Columns>,
    /// The values, in column order.
    values: Vec<Value>,
}

/// The column names of a result set with a case-insensitive lookup index.
struct Columns {
    /// The names in `SELECT` order, as the engine reports them.
    names: Vec<String>,
    /// Lower-cased name to position.
    index: HashMap<String, usize>,
}

impl Columns {
    /// Builds the shared column index for a result set.
    ///
    /// Names are indexed lower-cased because SQL identifiers are case
    /// insensitive and callers often write `"ID"` for a column declared as
    /// `id`. The last occurrence of a duplicated name wins.
    fn new(names: Vec<String>) -> Arc<Self> {
        let index = names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.to_ascii_lowercase(), i))
            .collect();
        Arc::new(Self { names, index })
    }
}

impl fmt::Debug for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut m = f.debug_map();
        for (name, value) in self.columns.names.iter().zip(&self.values) {
            m.entry(name, value);
        }
        m.finish()
    }
}

/// A column lookup: by name (`&str`) or by position (`usize`).
pub trait ColumnIndex: fmt::Display + Copy {
    /// Resolves to a position in the row, or `None` when absent.
    fn resolve(self, row: &Row) -> Option<usize>;
}

impl ColumnIndex for usize {
    fn resolve(self, row: &Row) -> Option<usize> {
        (self < row.values.len()).then_some(self)
    }
}

impl ColumnIndex for &str {
    /// Resolves the name as given first, then lower-cased, so an exact
    /// match wins when a result set has names differing only in case.
    fn resolve(self, row: &Row) -> Option<usize> {
        row.columns
            .index
            .get(self)
            .or_else(|| row.columns.index.get(&self.to_ascii_lowercase()))
            .copied()
    }
}

impl Row {
    /// The column names in `SELECT` order.
    pub fn columns(&self) -> &[String] {
        &self.columns.names
    }

    /// The number of columns.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the row has no columns.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Whether a column exists.
    pub fn has(&self, column: &str) -> bool {
        column.resolve(self).is_some()
    }

    /// Decodes a column.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Decode`] when the column does not exist or the value
    /// cannot be converted to `T`.
    pub fn get<T: FromValue>(&self, column: impl ColumnIndex) -> Result<T> {
        let idx = column
            .resolve(self)
            .ok_or_else(|| Error::decode(column, T::TYPE_NAME, "no such column"))?;
        T::from_value(self.values[idx].clone(), &self.columns.names[idx])
    }

    /// Decodes a column, returning `None` when it does not exist.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Decode`] when the value cannot be converted to `T`.
    pub fn try_get<T: FromValue>(&self, column: impl ColumnIndex) -> Result<Option<T>> {
        match column.resolve(self) {
            None => Ok(None),
            Some(idx) => {
                T::from_value(self.values[idx].clone(), &self.columns.names[idx]).map(Some)
            }
        }
    }

    /// The raw storage value of a column, or `None` when absent.
    pub fn raw(&self, column: impl ColumnIndex) -> Option<&Value> {
        column.resolve(self).map(|i| &self.values[i])
    }

    /// Iterates over `(name, value)` pairs in column order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.columns
            .names
            .iter()
            .map(String::as_str)
            .zip(self.values.iter())
    }
}

/// A boxed, sendable stream of rows.
pub type RowStream<'a> = Pin<Box<dyn Stream<Item = Result<Row>> + Send + 'a>>;

/// Generates the execution primitives for one engine crate.
///
/// The embedded and the serverless clients share their method names and
/// value shapes, so one body serves both; only the crate path differs.
/// Each instance is a private module holding the conversions between
/// `turso_sql::Value` and the engine's value type and the primitives
/// [`Conn`] dispatches to.
macro_rules! engine_module {
    ($(#[$meta:meta])* $name:ident, $engine:ident) => {
        $(#[$meta])*
        mod $name {
            use std::sync::Arc;

            use $engine::params_from_iter;
            use turso_sql::{Statement, Value};

            use super::{Columns, ExecResult, Row, RowStream};
            use crate::error::Result;

            /// Converts an engine value into the SQL layer's value type.
            fn from_engine(value: $engine::Value) -> Value {
                match value {
                    $engine::Value::Null => Value::Null,
                    $engine::Value::Integer(n) => Value::Integer(n),
                    $engine::Value::Real(f) => Value::Real(f),
                    $engine::Value::Text(s) => Value::Text(s),
                    $engine::Value::Blob(b) => Value::Blob(b),
                }
            }

            /// Converts a SQL layer value into the engine's value type.
            fn to_engine(value: Value) -> $engine::Value {
                match value {
                    Value::Null => $engine::Value::Null,
                    Value::Integer(n) => $engine::Value::Integer(n),
                    Value::Real(f) => $engine::Value::Real(f),
                    Value::Text(s) => $engine::Value::Text(s),
                    Value::Blob(b) => $engine::Value::Blob(b),
                }
            }

            /// The statement's bound values as engine parameters, in
            /// placeholder order.
            fn params(statement: &Statement) -> Vec<$engine::Value> {
                statement.values.iter().cloned().map(to_engine).collect()
            }

            /// Copies an engine row into an owned [`Row`] sharing `columns`.
            fn row_from(columns: &Arc<Columns>, row: &$engine::Row) -> Result<Row> {
                let mut values = Vec::with_capacity(columns.names.len());
                for i in 0..columns.names.len() {
                    values.push(from_engine(row.get_value(i)?));
                }
                Ok(Row {
                    columns: Arc::clone(columns),
                    values,
                })
            }

            /// Prepares `sql` through the connection's statement cache.
            async fn prepare(conn: &$engine::Connection, sql: &str) -> Result<$engine::Statement> {
                Ok(conn.prepare_cached(sql).await?)
            }

            /// Runs a query and collects every row.
            pub(super) async fn query_all(
                conn: &$engine::Connection,
                statement: &Statement,
            ) -> Result<Vec<Row>> {
                let mut stmt = prepare(conn, &statement.sql).await?;
                let mut rows = stmt.query(params_from_iter(params(statement))).await?;
                let columns = Columns::new(rows.column_names());
                let mut out = Vec::new();
                while let Some(row) = rows.next().await? {
                    out.push(row_from(&columns, &row)?);
                }
                Ok(out)
            }

            /// Runs a query and returns its first row, if any.
            pub(super) async fn query_one(
                conn: &$engine::Connection,
                statement: &Statement,
            ) -> Result<Option<Row>> {
                let mut stmt = prepare(conn, &statement.sql).await?;
                let mut rows = stmt.query(params_from_iter(params(statement))).await?;
                let columns = Columns::new(rows.column_names());
                let first = rows.next().await?;
                // The remaining rows are drained so that the cached
                // statement is left fully stepped rather than holding an
                // open cursor, and its implicit read transaction, until it
                // is next reused.
                while rows.next().await?.is_some() {}
                first.map(|row| row_from(&columns, &row)).transpose()
            }

            /// Runs a statement that returns no rows and reads the change
            /// counters.
            pub(super) async fn execute(
                conn: &$engine::Connection,
                statement: &Statement,
            ) -> Result<ExecResult> {
                let mut stmt = prepare(conn, &statement.sql).await?;
                let rows_affected = stmt.execute(params_from_iter(params(statement))).await?;
                Ok(ExecResult {
                    last_insert_id: conn.last_insert_rowid(),
                    rows_affected,
                })
            }

            /// Runs one or more `;`-separated statements without
            /// parameters. The batch API reports no change count.
            pub(super) async fn execute_unprepared(
                conn: &$engine::Connection,
                sql: &str,
            ) -> Result<ExecResult> {
                conn.execute_batch(sql).await?;
                Ok(ExecResult {
                    last_insert_id: conn.last_insert_rowid(),
                    rows_affected: 0,
                })
            }

            /// Runs a single parameterless statement, for transaction
            /// control.
            pub(super) async fn execute_raw(conn: &$engine::Connection, sql: &str) -> Result<()> {
                conn.execute(sql, ()).await?;
                Ok(())
            }

            /// Sets a pragma on the connection.
            pub(super) async fn pragma_update(
                conn: &$engine::Connection,
                name: &str,
                value: &str,
            ) -> Result<()> {
                conn.pragma_update(name, value).await?;
                Ok(())
            }

            /// Runs a query and streams its rows lazily; `holder` travels
            /// inside the stream and is dropped with it.
            pub(super) async fn stream<'a, H: Send + 'a>(
                conn: &$engine::Connection,
                statement: &Statement,
                holder: H,
            ) -> Result<RowStream<'a>> {
                let mut stmt = prepare(conn, &statement.sql).await?;
                let rows = stmt.query(params_from_iter(params(statement))).await?;
                let columns = Columns::new(rows.column_names());
                Ok(Box::pin(futures_util::stream::unfold(
                    (rows, columns, holder),
                    |(mut rows, columns, holder)| async move {
                        match rows.next().await {
                            Ok(Some(row)) => {
                                Some((row_from(&columns, &row), (rows, columns, holder)))
                            }
                            Ok(None) => None,
                            Err(e) => Some((Err(e.into()), (rows, columns, holder))),
                        }
                    },
                )))
            }
        }
    };
}

engine_module!(embedded, turso);
engine_module!(
    #[cfg(feature = "serverless")]
    remote,
    turso_serverless
);

/// An engine connection: embedded, or an HTTP session to Turso Cloud.
///
/// Both variants are cheap to clone, and clones share one engine
/// connection, which is why the pool never multiplies slots by cloning.
#[derive(Clone)]
pub(crate) enum Conn {
    /// A connection of the embedded engine: in-memory, file or replica.
    Embedded(turso::Connection),
    /// A session of the serverless client, one HTTP request per statement.
    #[cfg(feature = "serverless")]
    Remote(turso_serverless::Connection),
}

/// Dispatches one primitive call to the engine behind a [`Conn`].
macro_rules! dispatch {
    ($conn:expr, |$c:ident| $call:expr) => {
        match $conn {
            Conn::Embedded($c) => {
                use embedded as engine;
                $call
            }
            #[cfg(feature = "serverless")]
            Conn::Remote($c) => {
                use remote as engine;
                $call
            }
        }
    };
}

impl Conn {
    /// Whether no explicit transaction is open on this connection.
    ///
    /// # Errors
    ///
    /// Returns the engine error when the state cannot be read.
    pub(crate) fn is_autocommit(&self) -> Result<bool> {
        match self {
            Conn::Embedded(c) => Ok(c.is_autocommit()?),
            #[cfg(feature = "serverless")]
            Conn::Remote(c) => Ok(c.is_autocommit()?),
        }
    }

    /// Sets the engine's own lock wait — embedded engine only, since an
    /// HTTP session has no local lock to wait on.
    ///
    /// # Errors
    ///
    /// Returns the engine error when the timeout is rejected.
    pub(crate) fn busy_timeout(&self, timeout: Duration) -> Result<()> {
        match self {
            Conn::Embedded(c) => Ok(c.busy_timeout(timeout)?),
            #[cfg(feature = "serverless")]
            Conn::Remote(_) => Ok(()),
        }
    }

    /// Sets a pragma on the connection.
    ///
    /// # Errors
    ///
    /// Returns the engine error when the pragma is rejected.
    pub(crate) async fn pragma_update(&self, name: &str, value: &str) -> Result<()> {
        dispatch!(self, |c| engine::pragma_update(c, name, value).await)
    }

    /// Runs a single parameterless statement, for transaction control.
    ///
    /// # Errors
    ///
    /// Returns the engine error when the statement fails.
    pub(crate) async fn execute_raw(&self, sql: &str) -> Result<()> {
        dispatch!(self, |c| engine::execute_raw(c, sql).await)
    }
}

/// Runs a query and collects every row.
///
/// # Errors
///
/// Returns the engine error when the statement cannot be prepared, bound or
/// stepped.
pub(crate) async fn query_all(conn: &Conn, statement: &Statement) -> Result<Vec<Row>> {
    tracing::debug!(sql = %statement.sql, "query_all");
    dispatch!(conn, |c| engine::query_all(c, statement).await)
}

/// Runs a query and returns its first row, if any.
///
/// # Errors
///
/// Returns the engine error when the statement cannot be prepared, bound or
/// stepped.
pub(crate) async fn query_one(conn: &Conn, statement: &Statement) -> Result<Option<Row>> {
    tracing::debug!(sql = %statement.sql, "query_one");
    dispatch!(conn, |c| engine::query_one(c, statement).await)
}

/// Runs a statement that returns no rows and reads the change counters.
///
/// # Errors
///
/// Returns the engine error when the statement cannot be prepared, bound or
/// executed — with [`ErrorKind::Constraint`](crate::ErrorKind::Constraint)
/// on a constraint violation.
pub(crate) async fn execute(conn: &Conn, statement: &Statement) -> Result<ExecResult> {
    tracing::debug!(sql = %statement.sql, "execute");
    dispatch!(conn, |c| engine::execute(c, statement).await)
}

/// Runs one or more `;`-separated statements without parameters.
///
/// The engines' batch API does not report a change count, so
/// `rows_affected` is always `0` here; `last_insert_id` is still read after
/// the batch.
///
/// # Errors
///
/// Returns the engine error when any statement of the batch fails.
pub(crate) async fn execute_unprepared(conn: &Conn, sql: &str) -> Result<ExecResult> {
    tracing::debug!(sql, "execute_unprepared");
    dispatch!(conn, |c| engine::execute_unprepared(c, sql).await)
}

/// Runs a query and streams its rows lazily.
///
/// `holder` is any value that must stay alive while the stream is consumed,
/// for example the pooled connection; it is moved into the stream's state
/// and dropped with it.
///
/// # Errors
///
/// Returns the engine error when the statement cannot be prepared or
/// started. Errors while stepping are yielded as stream items.
pub(crate) async fn stream<'a, H: Send + 'a>(
    conn: &Conn,
    statement: &Statement,
    holder: H,
) -> Result<RowStream<'a>> {
    tracing::debug!(sql = %statement.sql, "stream");
    dispatch!(conn, |c| engine::stream(c, statement, holder).await)
}
