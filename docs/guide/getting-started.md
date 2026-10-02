# Getting started

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

The first decision is where the data lives. Four modes, one line each:

=== "In memory"

    ```rust
    use turso_orm::prelude::*;

    let db = Database::connect(ConnectOptions::in_memory()).await?;
    ```

    A database that lives as long as the pool and vanishes with it. For
    tests, prototypes and short-lived agents.

=== "File"

    ```rust
    use turso_orm::prelude::*;

    let db = Database::connect(ConnectOptions::new("app.db")).await?;
    ```

    A file next to the binary, created on first open. For desktop tools,
    services with local state and anything that must survive a restart.

=== "Turso Cloud, replica"

    ```rust
    use turso_orm::prelude::*;

    let db = Database::connect(
        ConnectOptions::sync("replica.db", "libsql://<db>.turso.io").auth_token(token),
    )
    .await?;
    ```

    A local file kept in sync with a Turso Cloud database: reads stay
    local, writes are forwarded. Needs the `sync` feature.

=== "Turso Cloud, HTTP"

    ```rust
    use turso_orm::prelude::*;

    let db = Database::connect(
        ConnectOptions::remote("libsql://<db>.turso.io").auth_token(token),
    )
    .await?;
    ```

    No disk at all: every statement goes over HTTP. For functions at the
    edge. Needs the `serverless` feature.

`Database` is a pool of engine connections, cheap to clone and safe to
share. Entities, queries, transactions and migrations are the same in all
four modes; only this line changes. Tokens, encryption and the other
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
db.execute(
    Schema::new()
        .create_table_from_entity(user::Entity)
        .to_statement(),
)
.await?;
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
[examples on GitHub](https://github.com/aartintelligent/turso-orm/tree/main/examples)
show complete programs, including two web APIs.
