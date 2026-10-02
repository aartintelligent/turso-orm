//! SQL AST and query builder for the Turso dialect.
//!
//! This crate is the SQL layer of [turso-orm]. It owns the abstract syntax
//! tree of a statement — identifiers, values, expressions, DML and DDL
//! builders — and the single writer that renders that tree to SQL text with
//! `?` placeholders and the values to bind. It deliberately targets one
//! dialect, the SQLite grammar as implemented by Turso, which is why it stays
//! small, has no runtime dependency and binds parameters directly as Turso
//! storage-class [`Value`]s rather than through a generic value model.
//!
//! The crate does not execute anything: it never touches a connection and
//! knows nothing about rows. Executing a [`Statement`] is the job of
//! `turso-orm-driver`, and mapping entities onto statements is the job of
//! `turso-orm`.
//!
//! - [`Iden`], [`Ident`], [`TableRef`], [`ColumnRef`]: identifiers, always
//!   rendered double-quoted so reserved words and mixed case are safe;
//! - [`Value`]: the five SQLite storage classes, with `From` conversions for
//!   the common Rust types and, behind feature flags, `chrono`, `uuid`,
//!   `serde_json` and `rust_decimal`;
//! - [`Expr`] and [`Condition`]: expressions and composable `AND` / `OR`
//!   groups that parenthesise themselves so precedence is never ambiguous;
//! - [`Query`] and [`Table`]: entry points to the DML and DDL builders;
//! - [`Build`] and [`Statement`]: rendering to SQL plus bound values.
//!
//! # Example
//!
//! ```
//! use turso_sql::prelude::*;
//!
//! let (sql, values) = Query::select()
//!     .column("id")
//!     .column("name")
//!     .from("user")
//!     .and_where(Expr::col("age").gte(18))
//!     .order_by("name", Order::Asc)
//!     .limit(10)
//!     .build();
//! assert_eq!(
//!     sql,
//!     r#"SELECT "id", "name" FROM "user" WHERE "age" >= ? ORDER BY "name" ASC LIMIT ?"#
//! );
//! assert_eq!(values, vec![Value::Integer(18), Value::Integer(10)]);
//! ```
//!
//! [turso-orm]: https://github.com/aartintelligent/turso-orm
#![cfg_attr(docsrs, feature(doc_cfg))]

mod expr;
mod iden;
mod query;
mod schema;
mod value;
mod writer;

pub use expr::{Condition, Expr, Func, IntoCondition, Order};
pub use iden::{ColumnRef, Iden, Ident, IntoIden, TableRef};
pub use query::{
    Delete, Insert, Join, JoinType, OnConflict, Query, Returning, Select, SelectItem, Update,
};
pub use schema::{
    AlterTable, ColumnDef, ColumnType, CreateIndex, CreateTable, DropIndex, DropTable, ForeignKey,
    ForeignKeyAction, Table, TableConstraint,
};
#[cfg(feature = "with-chrono")]
#[cfg_attr(docsrs, doc(cfg(feature = "with-chrono")))]
pub use value::NAIVE_DATETIME_FORMAT;
pub use value::Value;
pub use writer::{Build, Statement};

/// Everything needed to build queries, for a single glob import.
///
/// The prelude re-exports every builder, expression and identifier type of
/// the crate so that application code can write `use turso_sql::prelude::*;`
/// and have the whole query language in scope.
pub mod prelude {
    pub use crate::{
        AlterTable, Build, ColumnDef, ColumnRef, ColumnType, Condition, CreateIndex, CreateTable,
        Delete, DropIndex, DropTable, Expr, ForeignKey, ForeignKeyAction, Func, Iden, Ident,
        Insert, IntoCondition, IntoIden, Join, JoinType, OnConflict, Order, Query, Returning,
        Select, SelectItem, Statement, Table, TableConstraint, TableRef, Update, Value,
    };
}
