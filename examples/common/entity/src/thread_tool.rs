//! The junction between `thread` and `tool`: a composite key and a
//! `belongs_to` towards each side, which is all a many-to-many relation
//! needs.

use turso_orm::prelude::*;

/// A row of the `thread_tool` table: one tool invoked in one thread.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[turso(table_name = "thread_tool")]
pub struct Model {
    #[turso(primary_key)]
    pub thread_id: i32,
    #[turso(primary_key)]
    pub tool_id: i32,
}

/// The two sides of the junction.
#[derive(Copy, Clone, Debug, DeriveRelation)]
pub enum Relation {
    #[turso(
        belongs_to = "super::thread::Entity",
        from = "Column::ThreadId",
        to = "super::thread::Column::Id",
        on_delete = "Cascade"
    )]
    Thread,
    #[turso(
        belongs_to = "super::tool::Entity",
        from = "Column::ToolId",
        to = "super::tool::Column::Id",
        on_delete = "Cascade"
    )]
    Tool,
}

impl ActiveModelBehavior for ActiveModel {}
