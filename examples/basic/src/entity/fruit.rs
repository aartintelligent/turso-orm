//! The `fruit` entity: optionally attached to a cake.

use turso_orm::prelude::*;

/// A row of the `fruit` table.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[turso(table_name = "fruit")]
pub struct Model {
    /// Auto-incremented key.
    #[turso(primary_key)]
    pub id: i32,
    /// The fruit name.
    pub name: String,
    /// The cake this fruit sits on, if any; detaching keeps the fruit.
    pub cake_id: Option<i32>,
}

/// The relations of `fruit`.
#[derive(Copy, Clone, Debug, DeriveRelation)]
pub enum Relation {
    /// The cake carrying the fruit; deleting the cake detaches the fruit.
    #[turso(
        belongs_to = "super::cake::Entity",
        from = "Column::CakeId",
        to = "super::cake::Column::Id",
        on_delete = "SetNull"
    )]
    Cake,
}

impl ActiveModelBehavior for ActiveModel {}
