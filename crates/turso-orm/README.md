# turso-orm

[![crates.io](https://img.shields.io/crates/v/turso-orm.svg)](https://crates.io/crates/turso-orm)
[![docs.rs](https://img.shields.io/docsrs/turso-orm)](https://docs.rs/turso-orm)

An async ORM dedicated to [Turso](https://github.com/tursodatabase/turso) with
a typed entity API: entities, active models, enum columns, queries with
offset and keyset pagination, partial models, relations (one-to-many,
many-to-many through a junction, self-references, multi-hop chains),
loaders, transactions and schema generation.

See the [workspace README](https://github.com/aartintelligent/turso-orm#readme)
for an overview and a quick start.

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
