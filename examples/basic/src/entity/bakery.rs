//! The `bakery` entity: the root of the schema, owning cakes.

use turso_orm::prelude::*;

/// A row of the `bakery` table.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[turso(table_name = "bakery")]
pub struct Model {
    /// Auto-incremented key.
    #[turso(primary_key)]
    pub id: i32,
    /// The bakery name, unique across the table.
    #[turso(unique)]
    pub name: String,
    /// Profit margin in percent.
    pub profit_margin: f64,
}

/// The relations of `bakery`.
#[derive(Copy, Clone, Debug, DeriveRelation)]
pub enum Relation {
    /// One bakery, many cakes; the join columns come from `cake`'s `belongs_to`.
    #[turso(has_many = "super::cake::Entity")]
    Cake,
}

impl ActiveModelBehavior for ActiveModel {}
