# turso-orm

[![crates.io](https://img.shields.io/crates/v/turso-orm.svg)](https://crates.io/crates/turso-orm)
[![docs.rs](https://img.shields.io/docsrs/turso-orm)](https://docs.rs/turso-orm)

An async ORM dedicated to [Turso](https://github.com/tursodatabase/turso) with
a typed entity API: entities, active models, enum columns, queries with
offset and keyset pagination, partial models, relations (one-to-many,
many-to-many through a junction, self-references, multi-hop chains),
loaders, transactions and schema generation.

See the [workspace README](https://github.com/aartintelligent/turso-orm#readme)
for a complete example.

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
