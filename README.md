<div align="center">

# Turso ORM

**Your entities, Turso underneath.**

An async Rust ORM dedicated to the [Turso database](https://turso.tech/):
in-process, SQLite-compatible, written in Rust, and now reachable from your
entities without a server, a C toolchain or a dialect switch.

[![CI](https://github.com/aartintelligent/turso-orm/actions/workflows/ci.yml/badge.svg)](https://github.com/aartintelligent/turso-orm/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/turso-orm.svg)](https://crates.io/crates/turso-orm)
[![docs.rs](https://img.shields.io/docsrs/turso-orm)](https://docs.rs/turso-orm)
[![Guide](https://img.shields.io/badge/guide-GitHub%20Pages-teal.svg)](https://aartintelligent.github.io/turso-orm/)
[![MSRV](https://img.shields.io/badge/MSRV-1.94-blue.svg)](Cargo.toml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

[Guide](https://aartintelligent.github.io/turso-orm/) ·
[Getting started](https://aartintelligent.github.io/turso-orm/guide/getting-started/) ·
[API reference](https://docs.rs/turso-orm) ·
[Examples](examples)

</div>

turso-orm is an independent, community-maintained project. It is not
affiliated with, sponsored or endorsed by Turso.

## About

Most Rust ORMs talk to several databases at once, and pay for it: every
engine's particularities hide behind a common abstraction, and the features
that make one engine interesting are the first to go. turso-orm makes the
opposite bet. It targets one engine and treats that as a feature.

Turso ships as a crate and runs inside your process. It speaks SQLite's
dialect and file format, and adds what SQLite does not have: concurrent
writers through MVCC, vector and full-text search, encryption at rest, and
embedded replicas of a cloud database. turso-orm puts a typed entity API on
top of all of it, with nothing lost in translation: if the builder offers it,
the engine runs it.

### Built for one engine

No backend enum, no dialect switch, no feature that exists only on paper.
`RETURNING` on every write, `INTEGER PRIMARY KEY` row ids, transactional DDL
and the four storage classes are relied upon, not papered over.

### Familiar entity API

A `Model` struct per table, generated `Entity`, `Column` and `ActiveModel`,
relations declared once and walked in every direction. If you have used an
ORM in Rust or elsewhere, the shape is the one you expect.

### Relations done right

One-to-many, many-to-many through a junction, self-references, several
relations to one table, and multi-hop chains, all with batch loaders that
answer one list with one query.

### Nothing to install

The engine is a dependency, not a service. Tests, examples and your own
suite run against an in-memory database that opens in milliseconds.

## A taste

```rust
use turso_orm::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[turso(table_name = "user")]
pub struct Model {
    #[turso(primary_key)]
    pub id: i32,
    #[turso(unique)]
    pub email: String,
}

#[derive(Copy, Clone, Debug, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

let db = Database::connect(ConnectOptions::in_memory()).await?;
let alice = ActiveModel { email: Set("alice@example.com".into()), ..Default::default() }
    .insert(&db)
    .await?;
let found = Entity::find().filter(Column::Email.ends_with("@example.com")).one(&db).await?;
```

The [getting started page](https://aartintelligent.github.io/turso-orm/guide/getting-started/)
turns this into a running program in five short steps.

## Where your data lives

| Mode | Open it with | For |
|---|---|---|
| In memory | `ConnectOptions::in_memory()` | Tests, prototypes, short-lived agents |
| A local file | `ConnectOptions::new("app.db")` | Desktop tools, services with local state |
| Turso Cloud, embedded replica | `ConnectOptions::sync(path, url)` | Fast local reads, writes synced back |
| Turso Cloud, over HTTP | `ConnectOptions::remote(url)` | Functions at the edge, no disk at all |

The entities, queries, transactions and migrations are the same in all four;
only that one line changes.

## What you get

- **Entities**: `DeriveEntityModel` generates the entity, columns, key and
  active model; composite keys; `bool`, integers, floats, text, blobs,
  `chrono`, `Uuid`, JSON, `Decimal` and your own enums as column types.
- **Writes**: `insert`, `update`, `save`, `delete` with `RETURNING`, bulk
  forms, upserts, hooks around every write, request structs and JSON bodies
  converted into active models.
- **Queries**: typed filters and conditions, ordering, offset and keyset
  pagination, `count`, `exists`, streaming, projections into partial models,
  tuples, JSON or any struct.
- **Relations**: `find_related`, `find_also_related`, `find_with_related`,
  joins with automatic aliasing, `Linked` chains, and loaders that avoid N+1
  queries.
- **Transactions**: `DEFERRED`, `IMMEDIATE`, `EXCLUSIVE` and Turso's
  `CONCURRENT`, nested through savepoints, rolled back on drop, with a
  closure form that commits for you.
- **Migrations**: versioned, each one committed with its bookkeeping row
  inside `BEGIN IMMEDIATE`; `up`, `down`, `status`, `fresh`, `refresh`,
  `reset`.
- **Errors you can match on**: unique, foreign-key, not-null and check
  violations and lock contention are classified once by the driver.
- **Turso specifics**: MVCC, vector and full-text functions, encryption at
  rest, embedded replicas, serverless over HTTP.

## Where it fits

- **Desktop and command-line tools** that keep state in a file next to the
  binary, with migrations shipped inside the program.
- **Web services** on axum or actix-web backed by a local database; the
  [examples](examples) show both, with a service layer tested in memory.
- **Edge and cloud** through Turso Cloud, as a replica or over HTTP, without
  changing the entity code.
- **AI agents** whose memory, tool calls and embeddings live in one database:
  structured output straight into rows, retrieval in the same transaction.
- **Tests** that open their own database in a few milliseconds and throw it
  away.

## Getting started

```toml
[dependencies]
turso-orm = "0.1"
tokio     = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Then follow the [guide](https://aartintelligent.github.io/turso-orm/): entities,
queries, relations, transactions and migrations, each page explaining the why
before the how. The [API reference](https://docs.rs/turso-orm) covers every
item.

| Example | What it shows |
|---|---|
| [`basic`](examples/basic) | A console walkthrough: schema generation, CRUD, filters, pagination, streaming, relations and loaders |
| [`axum_example`](examples/axum_example) | A JSON REST API on [axum](https://github.com/tokio-rs/axum) with `entity`, `migration` and `api` crates and a service-layer test |
| [`actix_example`](examples/actix_example) | The same API on [actix-web](https://actix.rs) |

```sh
cargo run --manifest-path examples/basic/Cargo.toml
cargo run --manifest-path examples/axum_example/Cargo.toml
```

## Status and roadmap

`0.1` covers entities, CRUD, relations, transactions and migrations against
in-memory, file, embedded-replica and serverless databases. The API may still
move where Turso's own features ask for a different shape; such changes are
called out in the changelogs.

Planned next: a command-line tool (migrations, entity generation from an
existing schema), typed helpers over the vector and full-text functions, and
nested writes of a model with its related rows. Not planned: a second
database engine, or a query language of its own.

## FAQ

**Is this an official Turso project?** No. turso-orm is independent and
community-maintained, and uses the Turso name only to say which database it
targets. The engine itself lives at
[tursodatabase/turso](https://github.com/tursodatabase/turso).

**Can it talk to PostgreSQL or MySQL?** No, and it will not. If you need
several databases, a multi-database ORM is the right tool; if you build on
Turso, this one is made for it.

**Is it a layer over SQLx or over another ORM?** No. The five crates are
written from scratch for this engine, from the SQL builder up, and the
stack is small enough to read.

**Is it production ready?** It is `0.1`. Every change runs the test suite
on Linux, macOS and Windows, the feature powerset, the MSRV, `cargo-deny`
and the documentation build; `unsafe` is forbidden across the workspace.
Read the roadmap above before betting a product on it.

## The crates

| Crate | Role |
|---|---|
| [`turso-orm`](crates/turso-orm) | Entities, active models, queries, relations, loaders, schema generation |
| [`turso-orm-migration`](crates/turso-orm-migration) | Versioned migrations with transactional DDL |
| [`turso-orm-driver`](crates/turso-orm-driver) | Connection pool, transactions with savepoints, streaming, typed rows |
| [`turso-sql`](crates/turso-sql) | SQL AST and builder for the Turso / SQLite dialect |
| [`turso-orm-macros`](crates/turso-orm-macros) | The derive macros |

Each crate depends only on the ones below it; the
[design page](https://aartintelligent.github.io/turso-orm/overview/design/)
records the layering and the decisions behind it.

## Minimum supported Rust version

Rust 1.94 (edition 2024). Bumping the MSRV is a minor-version change while
the crates are `0.x`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Development uses `just`:

```sh
just setup   # install the git hooks (formatting, clippy, Conventional Commits)
just test    # run tests against an in-memory Turso database
just ci      # fmt, clippy, tests, doctests, cargo-deny, examples, docs
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
