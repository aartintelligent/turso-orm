# turso-orm-driver

[![crates.io](https://img.shields.io/crates/v/turso-orm-driver.svg)](https://crates.io/crates/turso-orm-driver)
[![docs.rs](https://img.shields.io/docsrs/turso-orm-driver)](https://docs.rs/turso-orm-driver)

Connection pool, transactions with savepoints, row streaming and typed row
decoding over the [`turso`](https://crates.io/crates/turso) client, and
over [`turso_serverless`](https://crates.io/crates/turso_serverless) for
Turso Cloud behind the `serverless` feature. The
execution layer of [turso-orm](https://github.com/aartintelligent/turso-orm),
usable on its own.

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
