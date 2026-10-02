# Features

A tour of what the crates provide, in the order you meet them. Each row
links to the guide page that shows it in use.

## Entities and models

| Capability | Details |
|---|---|
| Derived entities | One `Model` struct yields `Entity`, `Column`, `PrimaryKey` and `ActiveModel`; composite keys of up to six columns. |
| Typed columns | `bool`, integers, floats, `String`, `Vec<u8>`, `chrono` dates and times, `Uuid`, JSON and `Decimal`, with `Option<T>` for nullable columns, and your own enums through `DeriveActiveEnum`. |
| Active values | `Set`, `Unchanged` and `NotSet` decide what each write touches; `insert`, `update`, `save` and `delete` return the stored row; `try_into_model` goes back to the model. |
| Request structs | `DeriveIntoActiveModel` turns a request body into an active model; `from_json` does the same from a JSON object. |
| Hooks | `before_save`, `after_save`, `before_delete`, `after_delete` on the active model. |
| Schema generation | `CREATE TABLE` and `CREATE INDEX` from the entities, `STRICT`-compatible, with foreign keys from the relations. |

[:octicons-arrow-right-24: Entities](../guide/entities.md)

## Queries

| Capability | Details |
|---|---|
| Filters | Typed operators on the generated `Column` enum: equality, ranges, `LIKE`, `IN`, subqueries, full-text `MATCH`. |
| Composition | `Condition::all()` and `any()`, ordering, limits, offsets, `GROUP BY` and `HAVING`. |
| Pagination and streaming | A paginator with page counts, a keyset cursor for stable API pages, and a row stream that holds one pooled connection. |
| Projections | Partial models that select their own columns, tuples by position, JSON objects, or any struct deriving `FromQueryResult`; `exists` for a yes-or-no. |
| Bulk writes | `insert_many`, `update_many` and `delete_many`, each with a `RETURNING` form. |
| Escape hatches | The underlying SQL builder is public, and raw statements decode into the same row type. |

[:octicons-arrow-right-24: Queries](../guide/queries.md)

## Relations

| Capability | Details |
|---|---|
| Declarations | `belongs_to`, `has_many` and `has_one`, with `ON DELETE` and `ON UPDATE` actions in the generated DDL; `via` for many-to-many through a junction. |
| Navigation | `find_related` in both directions, `inner_join` and `left_join`, a two-model select in a single query, grouped with `find_with_related`; self-references and several relations to one table through aliased joins. |
| Chains | `Linked` paths across any number of tables, with `find_linked`, `find_also_linked` and `find_with_linked`. |
| Loaders | `load_one`, `load_many` and `load_many_to_many` fetch the related rows of a whole slice with one query, in input order. |

[:octicons-arrow-right-24: Relations](../guide/relations.md)

## Transactions and connections

| Capability | Details |
|---|---|
| Transactions | `DEFERRED`, `IMMEDIATE`, `EXCLUSIVE` and Turso's `CONCURRENT` mode, nested through savepoints, with a closure form that commits or rolls back for you. |
| Pool | A small pool over the engine that enforces one task per connection, with acquire and busy timeouts and per-connection pragmas. |
| Connection options | In-memory or file databases, embedded replicas of Turso Cloud, or Turso Cloud over HTTP; read-only mode, encryption at rest, experimental engine flags. |
| Errors | One structured error type; constraint and busy conditions are classified by the driver. |

[:octicons-arrow-right-24: Transactions](../guide/transactions.md)

## Migrations

| Capability | Details |
|---|---|
| Versioned history | One module per migration, named by its version; a bookkeeping table, renamable per migrator, records what ran. |
| Atomic | Each migration and its bookkeeping row commit together inside `BEGIN IMMEDIATE`. |
| Operations | `up`, `down`, `status`, `refresh`, `fresh`, `reset`, plus catalog inspection with `has_table`, `has_column` and `has_index`. |
| DDL builders | Create, alter and drop tables and indexes, including Turso's `ALTER COLUMN`, `USING fts` and `STRICT`. |

[:octicons-arrow-right-24: Migrations](../guide/migrations.md)

## Turso specifics

| Capability | Details |
|---|---|
| Concurrent writers | `BEGIN CONCURRENT` under MVCC, with conflicts surfacing at commit as busy errors. |
| Vector search | `vector_distance_cos`, `vector_distance_l2` and `vector32` in the SQL builder. |
| Full-text search | `MATCH` on columns, `fts_score`, and `USING fts` indexes. |
| Replicas | `ConnectOptions::sync` opens an embedded replica of a Turso Cloud database, with `push` and `pull`. |
| Serverless | `ConnectOptions::remote` talks to Turso Cloud over HTTP, for environments without a disk. |
| Encryption | `ConnectOptions::encryption` for databases encrypted at rest. |

## The crates

| Crate | Role |
|---|---|
| [`turso-orm`](https://docs.rs/turso-orm) | Entities, active models, queries, relations, loaders, schema generation |
| [`turso-orm-migration`](https://docs.rs/turso-orm-migration) | Versioned migrations with transactional DDL |
| [`turso-orm-driver`](https://docs.rs/turso-orm-driver) | Pool, transactions with savepoints, streaming, typed rows |
| [`turso-sql`](https://docs.rs/turso-sql) | SQL AST and builder for the Turso / SQLite dialect |
| [`turso-orm-macros`](https://docs.rs/turso-orm-macros) | The derive macros |

Each crate depends only on the ones below it; the
[design page](design.md) explains the split.
