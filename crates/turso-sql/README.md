# turso-sql

[![crates.io](https://img.shields.io/crates/v/turso-sql.svg)](https://crates.io/crates/turso-sql)
[![docs.rs](https://img.shields.io/docsrs/turso-sql)](https://docs.rs/turso-sql)

SQL AST and query builder for the Turso / SQLite dialect, including Turso
extensions (`ALTER COLUMN`, `STRICT`, vector and full-text functions). No
runtime dependency; parameters are bound as SQLite storage classes.

## Part of turso-orm

This crate is one layer of
[turso-orm](https://github.com/aartintelligent/turso-orm), an async ORM for
the Turso database. The repository holds the
[design notes](https://aartintelligent.github.io/turso-orm/overview/design/),
the [examples](https://github.com/aartintelligent/turso-orm/tree/main/examples)
and the [issue tracker](https://github.com/aartintelligent/turso-orm/issues).

## License

MIT OR Apache-2.0, see
[LICENSE-MIT](https://github.com/aartintelligent/turso-orm/blob/main/LICENSE-MIT)
and
[LICENSE-APACHE](https://github.com/aartintelligent/turso-orm/blob/main/LICENSE-APACHE).
