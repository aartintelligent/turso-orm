# About

turso-orm is an independent, open source ORM for the
[Turso database](https://turso.tech/), written in Rust and maintained by
[Aurelien Andre](https://github.com/aartintelligent).

## The idea

Turso turns SQLite into a modern engine, rewritten in Rust, with
concurrent writers, vector and full-text search, encryption and replicas.
Rust applications that want it deserve an ORM that knows it, instead of a
multi-database layer that hides it. turso-orm is that ORM: one engine, one
dialect, a typed entity API, and nothing between your code and the
database that does not need to be there.

## Relationship with Turso

turso-orm is not affiliated with, sponsored or endorsed by Turso. It is a
community project built on the public `turso` crate. The Turso name and
logo belong to Turso and appear on this site only to reference the
database, as its [brand page](https://turso.tech/brand) allows.

Questions about the engine itself belong to the
[Turso documentation](https://docs.turso.tech/) and the
[Turso repository](https://github.com/tursodatabase/turso).

## Principles

- **Correctness before convenience.** A wrong type is an error, a failed
  migration leaves nothing behind, a constraint violation is classified.
- **Small surface, full documentation.** Every public item has rustdoc,
  the guide shows every feature in use, and the examples are compiled by
  the test suite.
- **Written here.** No code is copied from another project; the stack is
  built for this engine.
- **Open by default.** MIT or Apache-2.0, developed in public, with a
  roadmap you can read and a contributing guide you can follow.

## Links

- [Repository](https://github.com/aartintelligent/turso-orm) and
  [issues](https://github.com/aartintelligent/turso-orm/issues)
- [API reference](https://docs.rs/turso-orm) on docs.rs
- [turso-orm on crates.io](https://crates.io/crates/turso-orm)
- [Releases](https://github.com/aartintelligent/turso-orm/releases)
