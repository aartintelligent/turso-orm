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

The first decision is where the data lives. Four modes, one
`ConnectOptions` each:

| Mode | Options | For |
|---|---|---|
| In memory | `ConnectOptions::in_memory()` | Tests, prototypes, short-lived agents: the database lives as long as the pool. |
| File | `ConnectOptions::new("app.db")` | Desktop tools, services with local state, anything that must survive a restart. |
| Turso Cloud, replica | `ConnectOptions::sync("replica.db", url).auth_token(token)` | A local file kept in sync with a Turso Cloud database: reads stay local, writes are forwarded. Needs the `sync` feature. |
| Turso Cloud, HTTP | `ConnectOptions::remote(url).auth_token(token)` | No disk at all, every statement over HTTP: functions at the edge. Needs the `serverless` feature. |

The basic example reads the mode from its settings and opens the
database in one function:

```rust
--8<-- "examples/basic/src/db.rs:connect"
```

`Database` is a pool of engine connections, cheap to clone and safe to
share. Entities, queries, transactions and migrations are the same in all
four modes; only this function changes. Tokens, encryption and the other
options are on the [compatibility page](../overview/compatibility.md#turso),
transactions under [Transactions](transactions.md).

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
