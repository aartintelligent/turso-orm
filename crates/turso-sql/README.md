# turso-sql

[![crates.io](https://img.shields.io/crates/v/turso-sql.svg)](https://crates.io/crates/turso-sql)
[![docs.rs](https://img.shields.io/docsrs/turso-sql)](https://docs.rs/turso-sql)

SQL AST and query builder for the Turso / SQLite dialect, including Turso
extensions (`ALTER COLUMN`, `STRICT`, vector and full-text functions). No
runtime dependency; parameters are bound as SQLite storage classes.

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
