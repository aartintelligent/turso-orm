# Why

Most Rust ORMs are built to talk to several databases at once. That is a
reasonable goal, and it has a cost: every engine's particularities must be
hidden behind a common abstraction, and the features that make one engine
interesting are the first to be left out.

turso-orm makes the opposite bet. It targets one engine, the
[Turso database](https://turso.tech/), and treats that as a feature.

## One engine, taken seriously

Turso is an in-process database, compatible with SQLite and rewritten in
Rust. It ships as a crate, runs inside your process, and adds things SQLite
does not have: concurrent writers through MVCC, vector search, full-text
search, encryption at rest and embedded replicas of a cloud database.

An ORM that knows it talks to Turso can expose all of that directly. It can
also rely on SQLite's own rules instead of the lowest common denominator:
`RETURNING` on every write, `INTEGER PRIMARY KEY` row ids, transactional
DDL, the four storage classes and the way they decode.

## What that changes for you

- **Nothing to install.** The engine is a dependency, not a service. The
  test suite, the examples and your own tests run against an in-memory
  database.
- **No abstraction tax.** There is no backend enum, no dialect switch and no
  feature that exists only on paper. If the builder offers it, the engine
  runs it.
- **Honest types.** Rows decode by the Rust type you ask for, with SQLite's
  own leniency; `Option<T>` means `NULL` and nothing else. A wrong type is
  an error, not a silent `None`.
- **Errors you can match on.** Unique, foreign-key, not-null and check
  violations, and lock contention, are classified once by the driver. Your
  handler asks `is_unique_violation()`; it never parses a message.
- **A small stack you can read.** Five crates, each depending only on the
  ones below it, every public item documented, no `unsafe` anywhere.

## What it is not

turso-orm is not a port of another ORM, not a layer over SQLx, and not a
multi-database framework in waiting. It will not grow a second engine. If
you need PostgreSQL or MySQL tomorrow, a multi-database ORM is the right
tool; if you build on Turso, this one is made for it.

[:octicons-arrow-right-24: See the features](features.md){ .md-button }
[:octicons-arrow-right-24: Get started](../guide/getting-started.md){ .md-button .md-button--primary }
