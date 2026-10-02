//! The primary key of an entity, modeled by [`PrimaryKeyTrait`].
//!
//! The derive macro generates a `PrimaryKey` enum with one variant per key
//! column and implements both traits here on it. Splitting the key out of
//! the `Column` enum lets the query builders iterate exactly the key columns
//! when rendering `WHERE pk = ?` conditions, and lets
//! [`PrimaryKeyTrait::ValueType`] be the field type for a single column or a
//! tuple for a composite key, so `find_by_id(1)` and
//! `find_by_id((1, "a".to_owned()))` both type-check.

use crate::types::{IntoValueTuple, TryFromU64};

use super::iden::{IdenStatic, Iterable};

/// The primary key of an entity, derived on the `PrimaryKey` enum.
pub trait PrimaryKeyTrait: IdenStatic + Iterable {
    /// The Rust type of the key: the field type for a single column, a tuple
    /// of field types for a composite key.
    type ValueType: IntoValueTuple + TryFromU64 + Clone + std::fmt::Debug + Send + Sync;

    /// Whether the database generates the key, as with `INTEGER PRIMARY KEY`.
    fn auto_increment() -> bool;
}

/// The mapping between primary-key variants and column variants.
pub trait PrimaryKeyToColumn {
    /// The entity's column enum.
    type Column;

    /// The column this key variant corresponds to.
    fn into_column(self) -> Self::Column;

    /// The key variant for `column`, or `None` when the column is not part of the key.
    fn from_column(column: Self::Column) -> Option<Self>
    where
        Self: Sized;
}
