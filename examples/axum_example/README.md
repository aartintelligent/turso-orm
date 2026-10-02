# Axum with turso-orm example

A JSON REST API for a `post` table: list with pagination, create, read,
update and delete. The database is a local Turso file, so there is nothing
to install.

1. Adjust `HOST`, `PORT` and `DATABASE_URL` in `.env` if needed. The URL is
   a file path; `:memory:` opens an in-memory database.
1. Run `cargo run` to apply the migrations and start the server.
1. Try it:

```sh
curl -s localhost:8000/posts
curl -s -X POST localhost:8000/posts -H 'content-type: application/json' \
     -d '{"title":"Hello","text":"First post"}'
curl -s localhost:8000/posts/1
curl -s -X PUT localhost:8000/posts/1 -H 'content-type: application/json' \
     -d '{"title":"Hello again","text":"Edited"}'
curl -s -X DELETE localhost:8000/posts/1
```

Run the service tests (they use an in-memory database):

```sh
cargo test -p axum-example-api
```

Run migrations by hand:

```sh
cargo run -p migration -- status
cargo run -p migration -- down
cargo run -p migration -- up
```
