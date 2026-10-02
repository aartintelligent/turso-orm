//! The service layer: every database access the handlers need, framework-free.

mod mutation;
mod query;

pub use mutation::Mutation;
pub use query::Query;
