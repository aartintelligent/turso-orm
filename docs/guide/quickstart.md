# Quickstart

The shortest path from an empty project to a first query. Each step names
the page that explains it in depth; this one only gets you running.

## Add the crates

```toml title="Cargo.toml"
[dependencies]
turso-orm = "0.1"
tokio     = { version = "1", features = ["macros", "rt-multi-thread"] }
```

The engine is part of the crate: there is nothing to install or start.
Feature flags are listed on the
[compatibility page](../overview/compatibility.md).

## Open a database

The first decision is where the data lives: in memory, in a local file, in
an embedded replica of a Turso Cloud database, or in Turso Cloud over HTTP.
One `ConnectOptions` constructor each, and `Database::connect` returns a
pool of engine connections, cheap to clone and safe to share. The program
below opens an in-memory database with `ConnectOptions::in_memory()`, which
is all a first run needs; the four modes, the pool's options and how a
replica exchanges data with the cloud are on the
[database page](database.md). Entities, queries, transactions and
migrations are the same in every mode.

## Describe a table

```rust
--8<-- "crates/turso-orm/examples/quickstart.rs:entity"
```

One struct per table, in a module named after it. The derive generates
`Entity`, `Column`, `PrimaryKey` and `ActiveModel` from it; the empty
`Relation` enum is where joins will be declared. Attributes and column
types are on the [entities page](entities.md).

## Create the table

```rust
--8<-- "crates/turso-orm/examples/quickstart.rs:schema"
```

Good enough for a first run and for tests. An application keeps a history
of its schema instead; see [Migrations](migrations.md).

## Write and read

```rust
--8<-- "crates/turso-orm/examples/quickstart.rs:query"
```

`Set` marks the fields to write and `insert` returns the stored row with
its generated id. `find` starts a query, `filter` narrows it with the
generated `Column` enum, `one` runs it. Everything the builder can do is
on the [queries page](queries.md), and the
[basic example on GitHub](https://github.com/aartintelligent/turso-orm/tree/main/examples/basic)
is a complete program on the same schema, a JSON API over a small agent
runtime: models, agents, threads, records and tools.
