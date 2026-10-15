# Observability

turso-orm reports what it does through [`tracing`](https://docs.rs/tracing):
every statement handed to the engine, every transaction boundary, every
connection the pool opens, every replica exchange with Turso Cloud and every
migration applied. Nothing is on by default, since `tracing` events go
nowhere without a subscriber; an application installs one and chooses what
to see with a filter. This page lists what there is to see and how the basic
example sets it up.

## Targets and levels

Each module of the driver emits under its own target, so that a subscriber
can follow one layer at a time or all of them together:

| Target | Level | Events |
|---|---|---|
| `turso_orm_driver::database` | `info` | The database is opened, with its source and pool size; a replica is pushed, pulled or checkpointed, with whether the pull changed anything. |
| `turso_orm_driver::database` | `debug` | A connection is opened or discarded; a replica's stats are read, with the pending count and the log size. |
| `turso_orm_driver::database` | `trace` | A connection is acquired or returned. |
| `turso_orm_driver::transaction` | `debug` | A transaction or a savepoint begins, commits or rolls back, with its depth. |
| `turso_orm_driver::transaction` | `warn` | A transaction is dropped without commit or rollback, or a nested one is rolled back for having been dropped. |
| `turso_orm_driver::executor` | `debug` | Every statement handed to the engine, including `BEGIN`, `COMMIT`, savepoints and pragmas. |
| `turso_orm_migration` | `info` | A migration is applied or reverted, by name. |

The split lets a filter say what it wants. `turso_orm_driver=debug` shows
everything the driver does, statements included;
`turso_orm_driver::transaction=debug` alone gives a timeline of
transactions without the statements inside them; `turso_orm_driver=info`
keeps the openings, the replica exchanges and the warnings, which is what a
production log usually wants.

The events carry the SQL text, never the bound values, so a log never holds
a row's content. The `Debug` form of `ConnectOptions`, `SyncOptions`,
`RemoteOptions` and `Encryption` replaces a token or a key with
`<redacted>`, so a `Database` or its options written to a log show where the
data lives and nothing a reader could reuse.

## The subscriber

The basic example installs a `tracing-subscriber` with a fixed filter: the
application at `info`, the driver at `debug`, and the engine's own
synchronisation log, which narrates every step of a pull at `info`, at
`warn` only:

```rust
--8<-- "examples/basic/src/telemetry.rs"
```

The example also wraps its router in `tower-http`'s `TraceLayer`, which
opens one span per HTTP request. The driver's events emitted while a
request is handled land inside that span, so a subscriber that prints span
context, or exports to a collector, ties each statement to the request that
caused it.

## What a request looks like

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

And a replica synchronised through the example's `POST /sync`, the
engine's own log muted:

```text
INFO turso_orm_driver::database: replica pushed
INFO turso_orm_driver::database: replica pulled changed=true
```

Each of these lines is one `tracing` event; a JSON subscriber or an
OpenTelemetry layer receives the same fields, `sql`, `mode`, `depth`,
`changed`, as structured data rather than text.
