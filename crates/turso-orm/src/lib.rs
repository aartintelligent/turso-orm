//! ORM: an async object-relational mapper dedicated to the [Turso] database.
//!
//! The public API follows the entity, active model and query builder shape
//! familiar from the Rust ORM ecosystem, but this is a from-scratch,
//! single-engine stack: there is no backend abstraction and no SQL dialect
//! switch. Targeting one engine lets the crate lean on SQLite semantics
//! directly — `INTEGER PRIMARY KEY` row ids,
//! `RETURNING *`, `pragma_table_info` — instead of papering over differences.
//!
//! The crate sits on two siblings it re-exports rather than wraps:
//!
//! - [`turso_orm_driver`] owns connections, pooling, transactions and row
//!   decoding; it is re-exported as [`Database`], [`ConnectionTrait`],
//!   [`Transaction`] and friends.
//! - [`turso_sql`] owns the SQL builders and the [`Value`] type; it is
//!   re-exported as [`sql`] together with the pieces entity code needs
//!   ([`Expr`], [`Condition`], [`Func`], [`Order`], [`Statement`]).
//!
//! What this crate adds is the entity layer ([`entity`]) — the traits a
//! `Model` struct, its `Column` and `PrimaryKey` enums and its `ActiveModel`
//! implement — and the typed query builders ([`query`]) that turn them into
//! statements. The derive macros that generate those impls live in
//! `turso_orm_macros` and are re-exported behind the `macros` feature.
//!
//! # Design decisions
//!
//! - Generated `Column` enums deliberately do not derive `PartialEq`, so that
//!   `Column::X.eq(v)` resolves to the [`entity::ColumnTrait`] condition
//!   builder instead of the `PartialEq` method.
//! - Every [`entity::ColumnTrait`] builder emits a qualified `table.column`
//!   reference, so conditions written against one entity never clash with a
//!   joined table that has a column of the same name.
//! - [`entity::ModelTrait::set`] and [`entity::ActiveModelTrait::set`] return
//!   a [`Result`] instead of panicking on a value of the wrong type.
//! - A relation is plain data ([`entity::RelationDef`]): the same value
//!   renders the join, drives the loaders and emits the foreign key. A
//!   many-to-many relation is two of them ([`entity::Related::via`]), a
//!   [`entity::Linked`] chain any number, and a table joined twice is
//!   aliased by [`query::Select`] rather than by the caller. Rust allows one
//!   `Related<Target>` impl per entity, so a second relation to the same
//!   table is used through its definition.
//! - The `__private` module exists for generated code only; nothing in it is
//!   part of the public API.
//!
//! # Example
//!
//! ```ignore
//! use turso_orm::prelude::*;
//!
//! #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
//! #[turso(table_name = "user")]
//! pub struct Model {
//!     #[turso(primary_key)]
//!     pub id: i32,
//!     #[turso(unique)]
//!     pub email: String,
//!     pub name: Option<String>,
//! }
//!
//! #[derive(Copy, Clone, Debug, DeriveRelation)]
//! pub enum Relation {}
//!
//! impl ActiveModelBehavior for ActiveModel {}
//!
//! # async fn run() -> Result<(), DbErr> {
//! let db = Database::connect(ConnectOptions::in_memory()).await?;
//! db.execute(Schema::new().create_table_from_entity(Entity).to_statement()).await?;
//! let user = ActiveModel { email: Set("a@b.c".into()), ..Default::default() }.insert(&db).await?;
//! let found = Entity::find_by_id(user.id).one(&db).await?;
//! # Ok(())
//! # }
//! ```
//!
//! [Turso]: https://github.com/tursodatabase/turso

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod entity;
mod error;
pub mod query;
pub mod types;

pub use entity::Schema;
pub use error::{DbErr, Result};

/// Items the derive macros expand to; not part of the public API.
///
/// Generated code refers to these through `::turso_orm::__private::...` so
/// that the helpers stay out of the documented surface and can change
/// without a semver bump.
#[doc(hidden)]
pub mod __private {
    pub use crate::entity::active_model::decode_field;
    pub use crate::entity::iden::Static;
    pub use crate::entity::model::get_field;
    pub use async_trait::async_trait;
    pub use turso_orm_driver::{Error as DriverError, FromValue, Result as DriverResult, Row};
    pub use turso_sql::{Ident, IntoIden};
}
pub use turso_orm_driver::{
    ConnectOptions, ConnectionTrait, Database, ExecResult, Row, StreamTrait, Transaction,
    TransactionMode, TransactionTrait,
};
pub use turso_sql::{
    self as sql, Build, Condition, Expr, Func, IntoCondition, JoinType, Order, Statement, Value,
};

/// Alias of [`Database`] for code that prefers the longer name.
pub type DatabaseConnection = Database;

#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
pub use turso_orm_macros::{
    DeriveActiveEnum, DeriveEntityModel, DeriveIden, DeriveIntoActiveModel, DerivePartialModel,
    DeriveRelation, FromQueryResult,
};

/// Everything an entity module needs, meant to be glob-imported.
///
/// The prelude gathers the entity traits, the query builders, the connection
/// types and the SQL building blocks, plus feature-gated aliases for the
/// column types that come from an external crate (`chrono`, `uuid`,
/// `serde_json`, `rust_decimal`).
pub mod prelude {
    pub use crate::entity::NotSet;
    pub use crate::entity::{
        ActiveEnum, ActiveModelBehavior, ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait,
        FromQueryResult, IntoActiveModel, IntoActiveValue, Linked, LoaderTrait, ModelTrait,
        PartialModelTrait, PrimaryKeyTrait, Related, RelationDef, RelationTrait, Schema, Set,
        TryIntoModel, Unchanged,
    };
    pub use crate::query::{Cursor, Paginator, Select, SelectTwo, SelectTwoMany};
    pub use crate::types::TursoType;
    pub use crate::{
        Build, Condition, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbErr,
        Expr, Func, Order, Statement, StreamTrait, Transaction, TransactionMode, TransactionTrait,
        Value,
    };
    // The derive shares its name with the trait of the same purpose; Rust keeps
    // macros in their own namespace, so both can be glob-imported together.
    #[cfg(feature = "macros")]
    pub use crate::{
        DeriveActiveEnum, DeriveEntityModel, DeriveIden, DeriveIntoActiveModel, DerivePartialModel,
        DeriveRelation, FromQueryResult,
    };
    pub use turso_sql::prelude::{ColumnType, ForeignKeyAction, JoinType};

    /// A naive timestamp without time zone.
    #[cfg(feature = "with-chrono")]
    pub type DateTime = chrono::NaiveDateTime;
    /// A timestamp in UTC.
    #[cfg(feature = "with-chrono")]
    pub type DateTimeUtc = chrono::DateTime<chrono::Utc>;
    /// A timestamp with a fixed offset.
    #[cfg(feature = "with-chrono")]
    pub type DateTimeWithTimeZone = chrono::DateTime<chrono::FixedOffset>;
    /// A calendar date.
    #[cfg(feature = "with-chrono")]
    pub type Date = chrono::NaiveDate;
    /// A time of day.
    #[cfg(feature = "with-chrono")]
    pub type Time = chrono::NaiveTime;
    /// A JSON document.
    #[cfg(feature = "with-json")]
    pub type Json = serde_json::Value;
    /// A UUID.
    #[cfg(feature = "with-uuid")]
    pub type Uuid = uuid::Uuid;
    /// An exact decimal number.
    #[cfg(feature = "with-rust_decimal")]
    pub type Decimal = rust_decimal::Decimal;
}
