//! The bakery schema: a bakery owns cakes and a cake carries fruits.
//!
//! Each entity lives in its own module, as `turso-orm` expects the
//! generated `Entity`, `Column`, `PrimaryKey` and `ActiveModel` items to
//! share a namespace with their `Model`. The re-exports give the rest of
//! the program short names for the entities.

pub mod bakery;
pub mod cake;
pub mod fruit;

pub use bakery::Entity as Bakery;
pub use cake::Entity as Cake;
pub use fruit::Entity as Fruit;
