# turso-orm

[![CI](https://github.com/aartintelligent/turso-orm/actions/workflows/ci.yml/badge.svg)](https://github.com/aartintelligent/turso-orm/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/turso-orm.svg)](https://crates.io/crates/turso-orm)
[![docs.rs](https://img.shields.io/docsrs/turso-orm)](https://docs.rs/turso-orm)
[![Guide](https://img.shields.io/badge/guide-GitHub%20Pages-teal.svg)](https://aartintelligent.github.io/turso-orm/)
[![MSRV](https://img.shields.io/badge/MSRV-1.94-blue.svg)](Cargo.toml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

An async ORM dedicated to [Turso](https://turso.tech/), the SQLite-compatible
database engine written in Rust. One engine, one dialect, no SQLx, no C build,
and a typed entity API so that existing ORM knowledge carries over.

turso-orm is an independent, community-maintained project. It is not
affiliated with, sponsored or endorsed by Turso.

| Crate | Purpose |
|---|---|
| [`turso-orm`](crates/turso-orm) | Entities, `ActiveModel`, queries, relations, loaders, schema generation |
| [`turso-orm-migration`](crates/turso-orm-migration) | Versioned migrations with transactional DDL |
| [`turso-orm-driver`](crates/turso-orm-driver) | Connection pool, transactions with savepoints, streaming, typed rows |
| [`turso-sql`](crates/turso-sql) | SQL AST and builder for the Turso / SQLite dialect |
| [`turso-orm-macros`](crates/turso-orm-macros) | `DeriveEntityModel`, `DeriveRelation`, `FromQueryResult`, ... |

The [guide](https://aartintelligent.github.io/turso-orm/) walks through entities,
queries, relations, transactions, migrations and the web framework examples;
[docs.rs](https://docs.rs/turso-orm) is the API reference.

## Quick start

```toml
[dependencies]
turso-orm = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
mod user {
    use turso_orm::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[turso(table_name = "user")]
    pub struct Model {
        #[turso(primary_key)]
        pub id: i32,
        #[turso(unique)]
        pub email: String,
        pub name: Option<String>,
    }

    #[derive(Copy, Clone, Debug, DeriveRelation)]
    pub enum Relation {
        #[turso(has_many = "super::post::Entity")]
        Post,
    }

    impl ActiveModelBehavior for ActiveModel {}
}

use turso_orm::prelude::*;

#[tokio::main]
async fn main() -> Result<(), DbErr> {
    let db = Database::connect(ConnectOptions::new("app.db")).await?;
    db.execute(Schema::new().create_table_from_entity(user::Entity).to_statement()).await?;

    let alice = user::ActiveModel {
        email: Set("alice@example.com".into()),
        ..Default::default()
    }
    .insert(&db)
    .await?;

    let found = user::Entity::find()
        .filter(user::Column::Email.contains("example"))
        .order_by_asc(user::Column::Id)
        .one(&db)
        .await?;
    assert_eq!(found.as_ref(), Some(&alice));

    let txn = db.begin().await?;
    user::Entity::delete_by_id(alice.id).exec(&txn).await?;
    txn.commit().await?;
    Ok(())
}
```

## Examples

Each directory under [`examples/`](examples) is a standalone workspace:

| Example | What it shows |
|---|---|
| [`basic`](examples/basic) | A console walkthrough of the entity API: schema generation, CRUD, filters, pagination, streaming, relations and loaders |
| [`axum_example`](examples/axum_example) | A JSON REST API on [axum](https://github.com/tokio-rs/axum) with `entity`, `migration` and `api` crates and a service-layer test |
| [`actix_example`](examples/actix_example) | The same API on [actix-web](https://actix.rs) |

```sh
cargo run --manifest-path examples/basic/Cargo.toml
cargo run --manifest-path examples/axum_example/Cargo.toml
```

## What you get

- **Entities** with `DeriveEntityModel`: `Entity`, `Column`, `PrimaryKey` and
  `ActiveModel` generated from the model struct; composite keys; `TursoType`
  mapping for `bool`, integers, floats, `String`, `Vec<u8>`, `chrono`,
  `Uuid`, JSON and `Decimal` fields, plus your own enums with
  `DeriveActiveEnum`.
- **CRUD** through `ActiveModel::insert` / `update` / `save` / `delete`,
  `insert_many`, `update_many`, `delete_many`, each with `RETURNING`;
  request structs with `DeriveIntoActiveModel`, JSON input with
  `from_json`, hooks around every write.
- **Queries**: `filter`, `Condition::all()` / `any()`, ordering, limits,
  offset and keyset pagination, `count`, `exists`, streaming, projections
  into partial models, tuples, JSON or any `FromQueryResult` struct.
- **Relations**: `DeriveRelation` with `belongs_to` / `has_many` /
  `has_one`, many-to-many through a junction with `via`, self-references,
  several relations to one table, multi-hop `Linked` chains;
  `find_related`, `find_also_related`, `find_with_related`, joins with
  automatic aliasing, and batch loaders (`load_one` / `load_many` /
  `load_many_to_many`) that avoid N+1 queries.
- **Transactions** pinned to one connection: `BEGIN DEFERRED` /
  `IMMEDIATE` / `EXCLUSIVE` / `CONCURRENT` (MVCC), savepoints for nesting,
  rollback on drop, busy retry with backoff.
- **Connection pool** over the Turso engine with per-connection pragmas, an
  in-memory database shared across the pool, acquire and busy timeouts.
- **Typed decoding** by the Rust type you ask for, so `Option<T>` never
  masks a type error and expressions decode naturally.
- **Structured errors**: unique / foreign-key / not-null / check
  violations and lock contention are classified, not string-matched by you.
- **Migrations** with `MigratorTrait`: each migration and its bookkeeping
  row run in one `BEGIN IMMEDIATE` transaction; `up`, `down`, `status`,
  `fresh`, `refresh`, `reset`; a renamable bookkeeping table; catalog
  inspection with `has_table`, `has_column`, `has_index`.
- **Turso features**: encryption at rest, experimental engine flags, MVCC,
  embedded replicas synchronised with Turso Cloud (`sync` feature), Turso
  Cloud over HTTP (`serverless` feature), vector and full-text functions in
  the SQL builder.

## Why a dedicated ORM

Multi-database ORMs keep their driver layers private and decode rows with
exact-variant conversions that SQLite's four storage classes cannot satisfy,
so every Turso feature would have to pass through an abstraction that does
not want it. This project builds the familiar pieces (entities, active
values, relations, migrations) on a stack made for one engine. The
[design page](https://aartintelligent.github.io/turso-orm/overview/design/) of the guide records the decisions.

## Status

`0.1` covers entities, CRUD, relations, transactions and migrations against
in-memory, file, embedded-replica and serverless databases. Planned next: a
CLI (`migrate`, entity generation from an existing schema) and vector and
FTS query helpers.

## Minimum supported Rust version

Rust 1.94 (edition 2024). Bumping the MSRV is a minor-version change while
the crates are `0.x`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Development uses `just`:

```sh
just setup   # install the git hooks (formatting, clippy, Conventional Commits)
just test    # run tests against an in-memory Turso database
just ci      # fmt, clippy, tests, doctests, cargo-deny, examples
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
