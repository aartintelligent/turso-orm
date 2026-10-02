//! Enum columns, modeled by [`ActiveEnum`].
//!
//! SQLite has no enum type, so an enum column is stored as the text or
//! integer each variant maps to. The trait records that mapping once, and
//! the `DeriveActiveEnum` macro implements it together with the three
//! conversions a column type needs — `Into<Value>` for binding,
//! `FromValue` for decoding and `TursoType` for the DDL — so that an enum
//! can be used as a model field, in a condition and in an active model
//! exactly like a primitive.
//!
//! The backing type is left to the enum: `String` keeps the database
//! readable, an integer keeps it compact and sortable.

use turso_orm_driver::FromValue;
use turso_sql::{ColumnType, Value};

use crate::Result;

/// A Rust enum stored as one of its variants' values.
///
/// Derived by `DeriveActiveEnum`; implementing it by hand also works for an
/// enum that needs a custom mapping.
pub trait ActiveEnum: Sized + Clone + Send + Sync + 'static {
    /// The type the variants map to, `String` or an integer.
    type Value: Into<Value> + FromValue + Clone + PartialEq + Send + Sync + 'static;

    /// The column type the backing value is declared as in DDL.
    const COLUMN_TYPE: ColumnType;

    /// The name used in error messages.
    const NAME: &'static str;

    /// The stored value of this variant.
    fn to_value(&self) -> Self::Value;

    /// The variant stored as `value`.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Type`](crate::DbErr::Type) when no variant maps to
    /// `value`.
    fn try_from_value(value: &Self::Value) -> Result<Self>;

    /// Every variant, in declaration order.
    fn values() -> Vec<Self>;

    /// The stored value of this variant as a bound SQL value.
    fn into_value(self) -> Value {
        self.to_value().into()
    }
}
