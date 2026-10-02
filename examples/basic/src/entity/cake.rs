//! The `cake` entity: belongs to a bakery and carries fruits.

use turso_orm::prelude::*;

/// A row of the `cake` table.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[turso(table_name = "cake")]
pub struct Model {
    /// Auto-incremented key.
    #[turso(primary_key)]
    pub id: i32,
    /// The cake name.
    pub name: String,
    /// Price as a floating point number; use `with-rust_decimal` for money in a real app.
    pub price: f64,
    /// Whether the cake is gluten free.
    pub gluten_free: bool,
    /// Free-form attributes stored as JSON.
    pub attributes: Option<Json>,
    /// The owning bakery; indexed because every lookup goes through it.
    #[turso(indexed)]
    pub bakery_id: i32,
}

/// The relations of `cake`.
#[derive(Copy, Clone, Debug, DeriveRelation)]
pub enum Relation {
    /// The owning bakery; deleting the bakery deletes its cakes.
    #[turso(
        belongs_to = "super::bakery::Entity",
        from = "Column::BakeryId",
        to = "super::bakery::Column::Id",
        on_delete = "Cascade"
    )]
    Bakery,
    /// One cake, many fruits.
    #[turso(has_many = "super::fruit::Entity")]
    Fruit,
}

impl ActiveModelBehavior for ActiveModel {}
