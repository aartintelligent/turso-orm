# Basic example

A JSON API on [axum](https://github.com/tokio-rs/axum) that exercises the
entity API end to end on a small agent runtime: models, agents, threads
with their records and the tools they invoked. Every feature of the ORM the guide explains has a route here:
inserts and updates, filters, pagination and keyset cursors, streaming,
projections, relations in every direction, the many-to-many junction, the
self-reference, chains across tables, transactions and cascades.

The schema and its history live in the crate: `entity/` holds one module
per table, six in all, and `migration/` one migration per table plus the
seed, with the `migrate` binary to run them by hand. The API is four
layers, one directory each, and inside each layer one module per table,
named after it, so that `grep thread_record` finds every piece of that
table:

| Layer | Knows about | Holds |
|---|---|---|
| `dto/` | JSON | the request bodies, the query parameters and the response shapes; `base/` holds the paging every listing shares |
| `service/` | the ORM | the functions that read and write the database on any connection, the pool or a caller's transaction; the guide includes them |
| `handler/` | axum and the services | one short function per route, reading its input through the `extract` wrappers |
| `router.rs` | the handlers | every path of the API, spelled nowhere else |

`error.rs` turns a `DbErr` and a refused request into an HTTP status in one
place, and `extract.rs` keeps axum's own rejections in the same `{"error"}`
envelope. Around the layers, `settings.rs` reads the configuration, `db.rs`
opens the database in the mode it names, `state.rs` carries the pool, the
settings and the replica's task into the handlers behind accessors, and
`telemetry.rs` sets up tracing. `GET /health` pings the database.

## Run it

```sh
cargo run
```

The migrations run at start-up, then the server listens on
`127.0.0.1:7407`, one tracing span per request and every SQL statement at
`debug`. The catalog is seeded by the migrations: two models.

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
| `DATABASE__SYNC_INTERVAL` | `30` | Seconds between two rounds of the replica's synchronisation task, for `sync` |

For example, to keep the data in a file:

```sh
DATABASE__MODE=file DATABASE__PATH=agents.db cargo run
```

To work against Turso Cloud, give the URL of the database and a database
token (not an API token of the account). `remote` sends every statement
over HTTP; `sync` keeps an embedded replica in the file, reads and writes
it locally, and exchanges data with the cloud only when asked to:

```sh
DATABASE__MODE=sync DATABASE__PATH=replica.db \
DATABASE__URL=turso://<database>.turso.io DATABASE__AUTH_TOKEN=<token> cargo run
```

The replica pulls once at start-up, then a background task pushes the
writes made through the API, pulls what other writers committed and
checkpoints, every `DATABASE__SYNC_INTERVAL` seconds, until `Ctrl-C` stops
the server and the task pushes one last time. `POST /sync` forces a round
now, `GET /sync/stats` reports what the last round saw, and
`POST /sync/checkpoint` folds the replica's log into its file by hand. The
other modes answer `409 Conflict` to the three:

```sh
curl -X POST localhost:7407/sync
curl localhost:7407/sync/stats
curl -X POST localhost:7407/sync/checkpoint
```

The server applies the migrations itself. To operate them by hand on that
file, `migrate` takes `up`, `down`, `status`, `fresh`, `refresh` or
`reset`:

```sh
DATABASE__MODE=file DATABASE__PATH=agents.db cargo run --bin migrate -- status
```

## Try it

With the server running, one request at a time.

Check that the database answers:

```sh
curl -i localhost:7407/health
```

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
with `oneshot` requests, without opening a port; `tests/migrations.rs`
applies and reverts the migrations. They are what CI runs.
