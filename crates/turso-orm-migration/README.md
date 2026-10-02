# turso-orm-migration

[![crates.io](https://img.shields.io/crates/v/turso-orm-migration.svg)](https://crates.io/crates/turso-orm-migration)
[![docs.rs](https://img.shields.io/docsrs/turso-orm-migration)](https://docs.rs/turso-orm-migration)

Versioned schema migrations for [turso-orm](https://github.com/aartintelligent/turso-orm).
Each migration runs with its bookkeeping row inside one `BEGIN IMMEDIATE`
transaction, so a failure leaves no partial schema.

## Part of turso-orm

This crate is one of the five that make up
[turso-orm](https://github.com/aartintelligent/turso-orm), an async Rust ORM
dedicated to the Turso database. Start with the
[documentation](https://aartintelligent.github.io/turso-orm/), which walks
from the first entity to relations, transactions and migrations. The
[examples](https://github.com/aartintelligent/turso-orm/tree/main/examples)
show complete programs, and the
[issue tracker](https://github.com/aartintelligent/turso-orm/issues) is the
place to report a problem.

## License

MIT OR Apache-2.0, see
[LICENSE-MIT](https://github.com/aartintelligent/turso-orm/blob/main/LICENSE-MIT)
and
[LICENSE-APACHE](https://github.com/aartintelligent/turso-orm/blob/main/LICENSE-APACHE).
