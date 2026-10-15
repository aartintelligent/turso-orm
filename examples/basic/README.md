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

`error.rs` turns a `DbErr` into an HTTP status in one place.

## Run it

```sh
cargo run                       # in memory; DATABASE_URL=agents.db keeps a file
```

The migrations run at start-up, then the server listens on
`127.0.0.1:3000`. A few requests to try, the catalog being seeded by the
migrations:

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
