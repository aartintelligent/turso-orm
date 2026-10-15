# Database

`Database` is the handle everything else takes: a pool of engine
connections, cheap to clone and safe to share between tasks. Where its data
lives is the one decision an application makes about it. This page covers
the four places it can live, the options of the pool, how an embedded
replica exchanges data with Turso Cloud, what changes when every statement
travels over HTTP, and what the driver reports through `tracing`.

## Four modes

One `ConnectOptions` constructor per mode:

| Mode | Options | Feature | For |
|---|---|---|---|
| In memory | `ConnectOptions::in_memory()` | | Tests, prototypes, short-lived agents: the database lives as long as the pool. |
| File | `ConnectOptions::new("app.db")` | | Desktop tools, services with local state, anything that must survive a restart. |
| Embedded replica | `ConnectOptions::sync("replica.db", url).auth_token(token)` | `sync` | A local file that mirrors a Turso Cloud database: reads and writes stay local, and the file exchanges changes with the cloud on request. |
| Turso Cloud over HTTP | `ConnectOptions::remote(url).auth_token(token)` | `serverless` | No disk at all, every statement is a request: functions at the edge, containers without a volume. |

The basic example reads the mode from its settings and opens the database
in one function:

```rust
--8<-- "examples/basic/src/db.rs:connect"
```

Entities, queries, transactions and migrations are the same in all four
modes; only this function changes. The URL of a Turso Cloud database is
accepted as `libsql://`, `turso://` or `https://`, and the token must be a
database token: an API token of the account is refused with `401
Unauthorized`.

## The pool

`Database::connect` opens the engine, then one connection, so that a bad
path, a wrong key or a rejected pragma fails there rather than on the first
query. Every other connection is opened on demand, up to the pool size, and
configured the same way: pragmas are per connection in SQLite, and a
borrower must not observe different behaviour depending on which slot it
gets. A connection goes back to the pool only when it is in autocommit
mode; one left inside a transaction is dropped instead and the engine rolls
it back, so nothing leaks into the next borrower.

| Option | Default | Meaning |
|---|---|---|
| `max_connections(n)` | 8 | The number of slots. The engine serialises work on one connection, so this is also the number of statements in flight. |
| `acquire_timeout(d)` | 30 s | How long a borrower waits for a free slot before `DbErr::Driver` with a pool timeout. |
| `busy_timeout(d)` | 5 s | How long the engine waits for a lock held by another connection, and how long the driver retries a `BEGIN IMMEDIATE` under contention. `None` disables both. |
| `foreign_keys(bool)` | on | Whether the engine enforces foreign keys, cascades included. |
| `mvcc(bool)` | off | `PRAGMA journal_mode = 'mvcc'`, the prerequisite of `BEGIN CONCURRENT`. Local engine only. |
| `pragma(name, value)` | | Any other pragma, applied to every connection in order. |
| `read_only(bool)` | off | Opens the file read-only. |
| `encryption(cipher, hexkey)` | | Encryption at rest of a local file. Not available together with a replica. |
| `experimental(flags)` | all off | The engine features still behind a switch: `WITHOUT ROWID`, generated columns, `ATTACH` and the others. |

`db.ping()` runs `SELECT 1` through the pool, which is what a health check
wants. The reasoning behind the pool, one task per connection and no
sharing, is on the [design page](../overview/design.md).

## Embedded replicas

With the `sync` feature, `ConnectOptions::sync(path, url)` opens a local
file that is a replica of a Turso Cloud database. On the first open of an
empty file the engine downloads the database; afterwards the file behaves
like any local database, and every read and write is local and fast.

Nothing moves on its own. Writes made through the replica stay in the file
until `Database::push` sends them, and writes made elsewhere are invisible
until `Database::pull` brings them down; the engine never synchronises in
the background. An application decides when, and the two calls are all
there is to it:

```rust
--8<-- "examples/basic/src/service/sync.rs:push_pull"
```

The example exposes that pair as one route, refused in the other modes
since a plain database has no replica:

```rust
--8<-- "examples/basic/src/handler/sync.rs:sync"
```

When to call them is the application's business. A service that owns its
data pushes after each batch of writes, or on a timer; one that shares the
database pulls before a read that must be fresh. The connection function
above pulls once when the replica is opened, so that a file reopened after
a while serves what the cloud holds now and not what it held at the last
run. `pull` answers whether it changed anything, which a periodic task can
use to decide whether to refresh what it cached.

!!! note "Two clients, one database"

    A replica and a connection over HTTP to the same database see each
    other's writes only across a `push` or a `pull`. The driver's
    end-to-end test in `crates/turso-orm-driver/tests/sync.rs` exercises
    exactly that: a row written in the replica reaches the HTTP side after
    `push`, a row written over HTTP reaches the replica after `pull`.

## Over HTTP

With the `serverless` feature, `ConnectOptions::remote(url)` talks to Turso
Cloud without a local file. The same pool, the same transactions and the
same rows apply: a connection is a server-side session that carries its
transaction across requests, pooled like a local one, and `BEGIN`,
`COMMIT` and savepoints behave as described under
[Transactions](transactions.md).

Two things change. Every statement is a request, so a round trip costs
what the network costs between the process and the region of the database,
and a loop of small queries is worth turning into a join or a batch. And
the busy timeout and MVCC are properties of a local engine: they are
accepted and ignored. A database encrypted with a customer-managed key
takes its key through `remote_encryption_key`.

## Observability

The driver emits `tracing` events under one target per module, so that a
subscriber can follow one layer at a time or all of them together:

| Target | Level | Events |
|---|---|---|
| `turso_orm_driver::database` | `info` | The database is opened, with its source and pool size; a replica is pushed or pulled, with whether the pull changed anything. |
| `turso_orm_driver::database` | `debug` | A connection is opened or discarded. |
| `turso_orm_driver::database` | `trace` | A connection is acquired or returned. |
| `turso_orm_driver::transaction` | `debug` | A transaction or a savepoint begins, commits or rolls back, with its depth. |
| `turso_orm_driver::transaction` | `warn` | A transaction is dropped without commit or rollback. |
| `turso_orm_driver::executor` | `debug` | Every statement handed to the engine, including `BEGIN`, `COMMIT`, savepoints and pragmas. |
| `turso_orm_migration` | `info` | A migration is applied or reverted. |

The example installs a `tracing-subscriber` with a fixed filter: the
application at `info`, the driver at `debug`, and the engine's own
synchronisation log, which narrates every step of a pull at `info`, at
`warn` only:

```rust
--8<-- "examples/basic/src/telemetry.rs"
```

With that filter, appending two records to a thread reads like this, the
transaction boundaries around the statements they wrap:

```text
DEBUG turso_orm_driver::executor: execute_raw sql=BEGIN DEFERRED
DEBUG turso_orm_driver::transaction: transaction begun mode=Deferred
DEBUG turso_orm_driver::executor: query_one sql=SELECT COUNT(*) AS "num_items" FROM "thread_record" WHERE "thread_record"."thread_id" = ?
DEBUG turso_orm_driver::executor: query_all sql=INSERT INTO "thread_record" ("thread_id", "seq", "role", "content", "tool_calls") VALUES (?, ?, ?, ?, ?), (?, ?, ?, ?, ?) RETURNING *
DEBUG turso_orm_driver::executor: execute_raw sql=COMMIT
DEBUG turso_orm_driver::transaction: transaction committed depth=0
```

The events carry the SQL text, never the bound values, and the `Debug`
form of the options replaces a token or a key with `<redacted>`, so a
`Database` or a `ConnectOptions` written to a log shows where the data
lives and nothing a reader could reuse.
