//! The typed query builders: select, insert, update, delete and pagination.
//!
//! Each builder wraps a `turso_sql` statement builder and a `PhantomData`
//! of the entity it targets, so that column arguments are checked against
//! the right `Column` enum and results decode into the right model. The
//! builders own the translation from entity concepts (active values,
//! primary keys, relations) into SQL; they do not own rendering, which is
//! `turso_sql`'s job, nor execution, which goes through the driver's
//! `ConnectionTrait`.
//!
//! - [`Select`], [`SelectTwo`], [`SelectTwoMany`], [`Selector`] and
//!   [`RawSelector`] read rows.
//! - [`Paginator`] pages over any selector by offset; [`Cursor`] by key.
//! - [`Insert`] and [`InsertMany`] write active models.
//! - [`UpdateOne`] and [`UpdateMany`] change rows.
//! - [`DeleteOne`] and [`DeleteMany`] remove rows.

mod cursor;
mod delete;
mod insert;
pub(crate) mod select;
mod update;

pub use cursor::Cursor;
pub use delete::{DeleteMany, DeleteOne, DeleteResult};
pub use insert::{Insert, InsertMany, InsertResult};
pub use select::{ModelStream, Paginator, RawSelector, Select, SelectTwo, SelectTwoMany, Selector};
pub use update::{UpdateMany, UpdateOne, UpdateResult};
