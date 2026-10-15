# Basic example

A JSON API on [axum](https://github.com/tokio-rs/axum) that exercises the
entity API end to end on the agent runtime schema of `examples/common`:
providers and models, agents, threads with their records and the tools they
invoked. Every feature of the ORM the guide explains has a route here:
inserts and updates, filters, pagination and keyset cursors, streaming,
projections, relations in every direction, the many-to-many junction, the
self-reference, chains across tables, transactions and cascades.

The crate has four layers, one directory each:

| Layer | Knows about | Holds |
|---|---|---|
| `dto/` | JSON | the request bodies, the query parameters and the response shapes |
| `service/` | the ORM | the functions that read and write the database; the guide includes them |
| `handler/` | axum and the services | one short function per route |
| `router.rs` | the handlers | every route of the API |

`error.rs` turns a `DbErr` into an HTTP status in one place. Around the
layers, `settings.rs` reads the configuration, `db.rs` opens the database
in the mode it names, `state.rs` carries the pool and the settings into
the handlers, and `telemetry.rs` sets up tracing.

## Run it

```sh
cargo run
```

The migrations run at start-up, then the server listens on
`127.0.0.1:7407`, one tracing span per request and every SQL statement at
`debug`. The catalog is seeded by the migrations: two providers, two
models.

The database is the only thing to configure. The defaults live in
`src/settings.rs`; `DATABASE__*` environment variables override them, and
no file is read.

| Variable | Default | Meaning |
|---|---|---|
| `DATABASE__MODE` | `memory` | `memory`, `file`, `sync` (a Turso Cloud replica) or `remote` (Turso Cloud over HTTP) |
| `DATABASE__PATH` | `basic.db` | The file, for `file` and `sync` |
| `DATABASE__URL` | | The Turso Cloud URL, for `sync` and `remote` |
| `DATABASE__AUTH_TOKEN` | | The Turso Cloud token, for `sync` and `remote` |
| `DATABASE__FOREIGN_KEYS` | `true` | Whether the engine enforces foreign keys |
| `DATABASE__MAX_CONNECTIONS` | `8` | The size of the pool |

For example, to keep the data in a file:

```sh
DATABASE__MODE=file DATABASE__PATH=agents.db cargo run
```

## Try it

With the server running, one request at a time.

List the seeded models whose name contains `mini`:

```sh
curl localhost:7407/models?name=mini
```

Create an agent on the first model; the response is the stored row with
its id:

```sh
curl -X POST localhost:7407/agents \
  -H 'content-type: application/json' \
  -d '{"name":"planner","model_id":1}'
```

Open a thread for that agent:

```sh
curl -X POST localhost:7407/threads \
  -H 'content-type: application/json' \
  -d '{"agent_id":1,"title":"Plan the launch"}'
```

Append two records to the thread, numbered after the last one in one
transaction:

```sh
curl -X POST localhost:7407/threads/1/records \
  -H 'content-type: application/json' \
  -d '[{"role":"user","content":"Plan the launch."},{"role":"assistant","content":"Three steps."}]'
```

Read the thread back with its records:

```sh
curl localhost:7407/threads/1
```

Every route is listed in `src/router.rs`.

## Test it

```sh
cargo test
```

`tests/api.rs` builds the router on an in-memory database and drives it
with `oneshot` requests, without opening a port. It is what CI runs.
