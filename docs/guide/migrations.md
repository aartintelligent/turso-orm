# Migrations

Generating the schema from the entities is enough for a prototype, but an
application that lives for a while needs a history: which changes were
applied, in what order, and how to apply the next one on a database that
already has data. That is what `turso-orm-migration` provides. This page
explains how migrations are declared, how the migrator applies them, why
each one is atomic, and how to run them.

## How it works

The migrator keeps a bookkeeping table in your database,
`turso_migrations(version TEXT PRIMARY KEY, applied_at INTEGER)`, with
one row per applied migration; `migration_table_name()` on the migrator
renames it, for a database shared by several migrators or migrated by
another tool before. Every operation starts by making sure that
table exists and reading it; then it walks your declared migrations in
order and runs the ones whose state does not match: `up` applies the
pending ones, `down` reverts the applied ones, newest first.

Each migration runs inside its own `BEGIN IMMEDIATE` transaction, and the
bookkeeping insert or delete goes through that same transaction before
the commit. Two consequences:

- Taking the write lock up front means a busy database fails at `BEGIN`,
  before any schema change, rather than halfway through.
- SQLite DDL is transactional, so if a migration fails, its tables, its
  indexes and its version row are all rolled back together. The database
  is left exactly as before, and the migrator can be run again after the
  fix. There is never a "version 3 applied but table missing" state.

## Declaring a migration

A migration is a module whose name carries its version, holding a struct
that implements `MigrationTrait`. The `DeriveMigrationName` derive takes
the migration name from the module path, so every struct can simply be
called `Migration` and the file name is the single source of truth for
ordering:

```rust
--8<-- "examples/axum_example/migration/src/m20240101_000001_create_post_table.rs"
```

The naming convention is `m<yyyymmdd>_<seq>_<what>`: a date, a sequence
number for several migrations on the same day, and a description. Keep
the three parts: the migrator compares names as strings to know which
ones ran.

`up` receives a `SchemaManager`, a thin view over the migration's
transaction. It offers the DDL operations and a few catalog lookups:

| Method | What it does |
|---|---|
| `create_table(Table::create()...)` | Runs a `CREATE TABLE` built with the SQL builder. |
| `alter_table(Table::alter()...)` | `ALTER TABLE`, including Turso's `ALTER COLUMN ... TO` and column renames. |
| `drop_table(Table::drop()...)` | `DROP TABLE`. |
| `create_index(CreateIndex::new()...)`, `drop_index(...)` | Indexes, including Turso's `USING fts`. |
| `has_table(name)`, `has_column(table, column)`, `has_index(table, name)` | Inspect the catalog through `sqlite_schema` and `pragma_table_info`. |
| `get_connection()` | The transaction itself, for anything else. |

`down` reverts what `up` did. It has a default implementation that fails
with `DbErr::Migration`, so a migration that cannot be reverted says so
explicitly instead of silently doing nothing; a `down` that is wrong is
worse than none.

## Data migrations

Because `get_connection()` hands out the migration's transaction, a
migration can also move or seed data, and it benefits from the same
atomicity. The entity API works on it as on any connection:

```rust
--8<-- "examples/axum_example/migration/src/m20240101_000002_seed_posts.rs"
```

Two details are worth noting. The seed uses `insert_many` through the
entity, so the column list comes from the model and cannot drift from the
code. And `down` deletes exactly the rows `up` inserted, filtered by
title, rather than emptying the table, which may hold user data by then.

!!! warning "A migration and the entity it uses can drift"
    A migration refers to the entity as it is *today*, but the migration
    describes the schema as it was *then*. If a later migration adds a
    required column to `post`, this seed would start failing on a fresh
    database. Data migrations through the entity are convenient early in a
    project; for a schema that keeps changing, prefer explicit SQL or a
    copy of the model frozen inside the migration module.

## The migrator

The migrator is a type that lists the migrations, oldest first. That
list is the only thing to implement; `up`, `down`, `status` and the rest
come from `MigratorTrait`:

```rust
--8<-- "examples/axum_example/migration/src/lib.rs"
```

| Method | Effect |
|---|---|
| `up(db, None)` / `up(db, Some(n))` | Applies every pending migration, or the first `n`. |
| `down(db, None)` / `down(db, Some(n))` | Reverts every applied migration, or the last `n`, newest first. |
| `status(db)` | Each declared migration with an `applied` flag, in order. |
| `check(db)` | The mismatches between the list and the bookkeeping table, without changing anything. |
| `refresh(db)` | `down` everything, then `up`: rebuilds the schema through the migrations. |
| `fresh(db)` | Drops every table including the bookkeeping one, then `up`. For development databases. |
| `reset(db)` | `down` everything and stop. |

## Validating the list

Before running anything, the migrator compares the list with the
bookkeeping table:

- **Two migrations with the same name** fail every operation, `status`
  included, before the database is touched: the bookkeeping table could
  not tell them apart. With `DeriveMigrationName` it only happens when one
  migration is listed twice.
- **An applied migration missing from the list** is a
  `MigrationIssue::Unknown`. It happens when an older binary runs against
  a database a newer one migrated, which is what rolling a deployment back
  does, or after a migration was renamed. The migrator leaves its row
  alone.
- **A pending migration listed before an applied one** is a
  `MigrationIssue::OutOfOrder`. It happens when two branches that each
  added a migration are merged: `up` applies it after the later one, which
  is fine as long as the two do not touch the same tables.

`up` logs each issue as a warning and carries on. Override `strict()` to
return `true` and it fails instead, before applying anything; `refresh`
then refuses an unknown migration before reverting anything. `down` never
checks them, so that the way back stays open. `check(db)` returns the
issues for a CI step or a status command to show.

## Running migrations

The migrator is a library: how and when it runs is your decision. The web
examples call `Migrator::up(&db, None)` at start-up, before the server
binds its port, which is right for a service that owns its database. They
also ship a small binary for operating by hand:

```rust
--8<-- "examples/axum_example/migration/src/main.rs"
```

Running migrations from a dedicated binary rather than at start-up is the
better fit when several instances share one database file, or when a
human should see the `status` before anything changes.

## Starting from the entities

The DDL builders and the entity schema generator produce the same
statements, so the first migration of a project can be written in terms
of the entities:

```rust
--8<-- "examples/basic/src/main.rs:schema"
```

In a migration, each `to_statement()` would be executed through
`manager.get_connection()`. From the second migration on, write the
change explicitly: the entity now describes the target state, not the
step from the previous one.

!!! note "Turso specifics"
    `CREATE VIEW IF NOT EXISTS` is not idempotent in Turso 0.8, and
    `PRAGMA defer_foreign_keys` and `foreign_key_check` are unsupported.
    Create referenced tables before the tables that reference them, and
    drop them in the reverse order in `down`.
