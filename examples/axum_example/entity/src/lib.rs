//! The entities of the axum example.
//!
//! Each entity is a module holding its `Model` and the items the derive
//! generates from it; the prelude re-exports the entity types under short
//! names for callers that only need `find`, `insert` and friends.

pub mod post;
pub mod prelude;
