//! The `post` entity.

use serde::{Deserialize, Serialize};
use turso_orm::prelude::*;

/// A row of the `post` table, also the JSON shape of the API.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveEntityModel)]
#[turso(table_name = "post")]
pub struct Model {
    /// Auto-incremented key; clients never send it, so deserialization skips it.
    #[turso(primary_key)]
    #[serde(skip_deserializing)]
    pub id: i32,
    /// The post title.
    pub title: String,
    /// The post body.
    pub text: String,
}

/// The relations of `post`; there are none.
#[derive(Copy, Clone, Debug, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
