# Turso ORM

**Your entities, Turso underneath.**

turso-orm lets a Rust application keep its data in the
[Turso database](https://turso.tech/) through entities, relations and
migrations, with everything the engine can do still within reach.

[![CI](https://github.com/aartintelligent/turso-orm/actions/workflows/ci.yml/badge.svg)](https://github.com/aartintelligent/turso-orm/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/turso-orm.svg)](https://crates.io/crates/turso-orm)
[![docs.rs](https://img.shields.io/docsrs/turso-orm)](https://docs.rs/turso-orm)
[![MSRV](https://img.shields.io/badge/MSRV-1.94-blue.svg)](Cargo.toml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

[Guide](https://aartintelligent.github.io/turso-orm/) ·
[API reference](https://docs.rs/turso-orm) ·
[Examples](examples)

turso-orm is an independent, community-maintained project. It is not
affiliated with, sponsored or endorsed by Turso, and uses the Turso name only
to say which database it is made for.

## Turso, in a few words

[Turso](https://turso.tech/) is a database engine written in Rust by
[Turso](https://github.com/tursodatabase/turso), compatible with SQLite: the
same SQL, the same file format, the same idea of a database that runs inside
your program rather than on a server next to it. On top of that inheritance
it brings what SQLite never had: several writers at once, search over vectors
and over text, encryption of the data at rest, and a cloud offering where a
local file stays in sync with a hosted database, or where a function with no
disk at all talks to it over the network.

For a Rust team this changes the shape of a project. The database is a
dependency in `Cargo.toml`, not a service to provision; the test suite runs
against a database that opens in memory in a few milliseconds; the same code
serves a desktop tool, a web service and a function at the edge, and the
features that used to need a separate search engine or a managed cluster are
now a query away.

## The goal of this stack

An ORM gives those features a home in application code: a struct per table,
relations declared once and walked in every direction, migrations that carry
the schema from one version to the next, transactions that read like
transactions. Most Rust ORMs offer that for several databases at once, and
pay for it: each engine's particularities hide behind a common abstraction,
and the features that make one engine interesting are the first to go.

turso-orm makes the opposite bet. It is written for Turso alone, from the SQL
builder up, so that nothing the engine offers has to be smuggled through an
abstraction that does not want it. The result is a small stack you can read,
with no second dialect, no backend switch and no feature that exists only on
paper: if the engine runs it, the ORM exposes it.

What that buys you, in practice:

- **Modelling that fits the domain.** One-to-many, many-to-many through a
  junction, a table that refers to itself, several relations between the same
  two tables, paths across several tables; your schema does not have to bend
  to the tool.
- **Lists without the N+1 trap.** Loading the related rows of a whole page of
  results is one query, whatever the page size.
- **Writes that say what they touch.** Each field knows whether it is to be
  written, kept or left to the database default, so an insert sends only what
  you set and an update only what changed.
- **Errors you can act on.** A duplicate key, a missing parent row or a busy
  database arrive as classified errors, not as messages to parse.
- **Migrations that cannot half-apply.** Each step and its bookkeeping commit
  together, so a failure leaves the database exactly as it was.
- **Turso's own strengths, first class.** Concurrent writers, vector and
  full-text search, encryption, embedded replicas and the serverless mode are
  used through the same entities as everything else.

## Where it fits

- **Desktop and command-line tools** that keep state in a file next to the
  binary, with the schema's history shipped inside the program.
- **Web services** backed by a local database, from an internal tool to a
  prototype that may never need more; the [examples](examples) show one on
  axum and one on actix-web.
- **Edge and cloud**, through Turso Cloud as a synced replica or over HTTP,
  without changing the application code.
- **AI agents** whose memory, tool calls and embeddings live in one place:
  structured output goes straight into rows, retrieval happens in the same
  transaction as the bookkeeping.
- **Tests** that open their own database, run, and throw it away.

## Getting started

```toml
[dependencies]
turso-orm = "0.1"
tokio     = { version = "1", features = ["macros", "rt-multi-thread"] }
```

The [guide](https://aartintelligent.github.io/turso-orm/) takes it from
there: a first entity in five short steps, then entities, queries, relations,
transactions and migrations, each page explaining the why before the how.
The [API reference](https://docs.rs/turso-orm) covers every item, and the
[examples](examples) are complete programs that CI compiles and runs.

## Status and roadmap

`0.1` covers entities, reads and writes, relations, transactions and
migrations against in-memory, file, embedded-replica and serverless
databases. The API may still move where Turso's own features ask for a
different shape; such changes are called out in the [changelog](CHANGELOG.md).

Planned next: a command-line tool for migrations and for generating entities
from an existing schema, typed helpers over the vector and full-text
functions, and nested writes of a model with its related rows. Not planned: a
second database engine, or a query language of its own.

## FAQ

**Is this an official Turso project?** No. turso-orm is independent and
community-maintained. The engine itself, its licence and its roadmap belong
to [Turso](https://github.com/tursodatabase/turso).

**Can it talk to PostgreSQL or MySQL?** No, and it will not. If you need
several databases, a multi-database ORM is the right tool; if you build on
Turso, this one is made for it.

**Is it a layer over another ORM?** No. The five crates are written from
scratch for this engine, from the SQL builder up.

**Is it production ready?** It is `0.1`. Every change runs the test suite on
Linux, macOS and Windows, every feature combination, the minimum Rust version,
a supply-chain audit and the documentation build; `unsafe` is forbidden across
the workspace. Read the roadmap above before betting a product on it.

**How is it organised?** Five crates, each depending only on the ones below
it: the SQL builder, the driver, the derive macros, the ORM and the
migrations. The
[design page](https://aartintelligent.github.io/turso-orm/overview/design/)
of the guide records the layering and the decisions behind it.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md): the gates, the hooks, the branching
model and the commit conventions. The minimum supported Rust version is 1.94;
raising it is a minor-version change while the crates are `0.x`.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
