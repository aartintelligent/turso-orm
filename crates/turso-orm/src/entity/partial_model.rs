//! Partial models, modeled by [`PartialModelTrait`].
//!
//! A partial model is a struct that reads a subset of an entity's columns,
//! or expressions computed from them, instead of the whole row. Where a
//! plain `FromQueryResult` struct only knows how to decode a row, a partial
//! model also knows which select list produces that row, so that
//! `Select::into_partial_model` can build the projection and the decoder
//! from one declaration. `DerivePartialModel` implements both traits from
//! the field list and its `#[turso(...)]` attributes.

use super::model::FromQueryResult;

/// A projection struct that selects its own columns.
pub trait PartialModelTrait: FromQueryResult {
    /// Adds to `select`, whose select list has been cleared, one aliased
    /// item per field, named after the field so that decoding finds it.
    fn select_cols(select: turso_sql::Select) -> turso_sql::Select;
}
