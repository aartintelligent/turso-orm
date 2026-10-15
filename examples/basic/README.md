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

The settings are defaults in code, overridden by `BASIC__*` environment
variables with `__` between levels; no file is read.

| Variable | Default | Meaning |
|---|---|---|
| `BASIC__DATABASE__MODE` | `memory` | `memory`, `file`, `sync` (a Turso Cloud replica) or `remote` (Turso Cloud over HTTP) |
| `BASIC__DATABASE__PATH` | `basic.db` | The file, for `file` and `sync` |
| `BASIC__DATABASE__URL` | | The Turso Cloud URL, for `sync` and `remote` |
| `BASIC__DATABASE__AUTH_TOKEN` | | The Turso Cloud token, for `sync` and `remote` |
| `BASIC__DATABASE__FOREIGN_KEYS` | `true` | Whether the engine enforces foreign keys |
| `BASIC__DATABASE__MAX_CONNECTIONS` | `8` | The size of the pool |
| `BASIC__SERVER__ADDR` | `127.0.0.1:3000` | The address to bind |
| `BASIC__LOG__FILTER` | `info,turso_orm_driver=debug` | The tracing filter; the driver logs every statement at `debug` |

```sh
BASIC__DATABASE__MODE=file BASIC__DATABASE__PATH=agents.db cargo run
BASIC__DATABASE__MODE=remote BASIC__DATABASE__URL=libsql://<db>.turso.io BASIC__DATABASE__AUTH_TOKEN=... cargo run
```

The migrations run at start-up, then the server listens on the configured
address, one tracing span per request. A few requests to try, the catalog
being seeded by the migrations:

```sh
curl -s localhost:3000/models?name=mini
curl -s -X POST localhost:3000/agents -H 'content-type: application/json' \
  -d '{"name":"planner","model_id":1}'
curl -s -X POST localhost:3000/threads -H 'content-type: application/json' \
  -d '{"agent_id":1,"title":"Plan the launch"}'
curl -s -X POST localhost:3000/threads/1/records -H 'content-type: application/json' \
  -d '[{"role":"user","content":"Plan the launch."},{"role":"assistant","content":"Three steps."}]'
curl -s localhost:3000/threads/1
```

## Test it

```sh
cargo test
```

`tests/api.rs` builds the router on an in-memory database and drives it
with `oneshot` requests, without opening a port. It is what CI runs.
