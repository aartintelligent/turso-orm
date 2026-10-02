---
name: entity-api
description: The shape of the generated entity API (Entity, Column, PrimaryKey, ActiveModel, Relation) and the gotchas of the derive macros and query builders of turso-orm. Load before writing an entity, a relation, a query, a loader, or before touching crates/turso-orm-macros or crates/turso-orm/src/{entity,query}.
---
# Entity API

## What `DeriveEntityModel` generates from `Model`

| Item | Role |
|---|---|
| `Entity` | Unit struct implementing `EntityTrait`; `Entity::find()`, `find_by_id`, `insert`, `insert_many`, `update_many`, `delete_by_id`, `delete_many`. |
| `Column` | Enum of columns implementing `ColumnTrait`; **must not derive `PartialEq`**, or `Column::X.eq(v)` resolves to `PartialEq::eq`. |
| `PrimaryKey` | Enum of key columns; `find_by_id` takes the value, or a tuple for a composite key. |
| `ActiveModel` | `Model` with `ActiveValue<T>` fields and a `Default` of all `NotSet`. |

`Relation` is written by hand and derives `DeriveRelation`; the generated
`Related<R>` impls power `find_related`, `find_also_related`,
`find_with_related`, the joins and the loaders. `has_many` / `has_one`
infer their columns from the reverse `belongs_to`, so declare the
`belongs_to` side with `from` and `to`. `has_many` + `via = "junction"`
is many-to-many: `Related::via()` is the hop to the junction, `to()` the
junction-to-target hop, both read off the junction's `belongs_to`s. One
`Related<Target>` impl per entity: the first variant naming a target wins;
the others go through `Relation::X.def()` with `Select::related_to`,
`join` or `join_as`. `Linked` (hand-written unit struct, `link()` returns
the hops) covers multi-hop paths: `find_linked`, `find_also_linked`,
`find_with_linked`.

Other derives: `DeriveActiveEnum` (`rs_type = "String"` or an integer;
`string_value` / `num_value` per variant; emits `ActiveEnum`, `Into<Value>`,
`FromValue`, `TursoType`), `DerivePartialModel` (`entity = "..."`;
`from_col` / `from_expr` per field; emits `PartialModelTrait` +
`FromQueryResult`), `DeriveIntoActiveModel` (`active_model = "..."`;
`ignore` per field; goes through `IntoActiveValue`, where `Option<T>` means
`Set`-or-`NotSet`). `DeriveEntityModel` also emits `TryIntoModel`.

Generated code reaches helpers through `turso_orm::__private`; the macros
are only usable via the `turso_orm` re-exports (feature `macros`).

## Attribute names

`table_name`, `primary_key`, `auto_increment`, `column_name`, `unique`,
`indexed`, `nullable`, `default_value`, `ignore`, `has_many`, `has_one`,
`belongs_to`, `from`, `to`, `via`, `on_delete`, `on_update`, `skip_fk`,
`rs_type`, `string_value`, `num_value`, `entity`, `from_col`, `from_expr`,
`active_model`. The parser lives in `crates/turso-orm-macros/src/attrs.rs`;
an unknown name is a spanned compile error on the user's attribute.

## Query builder facts

- Column references render qualified (`"table"."column"`), so joins never
  clash.
- `find_also_related` aliases columns `A_<col>` / `B_<col>`;
  `FromQueryResult::from_query_result(row, prefix)` takes that prefix and
  `from_query_result_optional` returns `None` when every prefixed column is
  `NULL`.
- `count` and `exists` strip ordering, limit and offset from the inner query.
- A table joined twice is aliased `table_n` by `Select::join_table`; the
  `ON` is rendered against the alias. `join_as` takes an explicit alias.
- `SelectTwoMany::all` groups pairs by the left primary key in order of
  first appearance; it has no `limit` on purpose.
- `cursor_by([cols])` takes an array; boundaries are a value or a tuple;
  `last(n)` reads reversed with `LIMIT` and flips in memory.
- `contains` / `starts_with` / `ends_with` emit `LIKE ? ESCAPE ?`.
- `stream` returns `ModelStream`, a boxed `Unpin` stream, so
  `while let Some(x) = s.try_next().await?` works without pinning.
- `Option<T>` decodes `NULL` only; a wrong storage class is `DbErr::Type`.
- DDL defaults are inlined as literals: SQLite does not bind parameters in
  DDL.

## Tests to extend

`crates/turso-orm/tests/entity.rs` holds `user` / `post` / `tag` entities
and one test per area (CRUD, queries, projections, relations and loaders,
composite keys and transactions); `crates/turso-orm/tests/relational.rs`
holds `user` / `post` / `category` / `post_category` / `quad` and one test
per relational feature (junction, self-reference, chains, enums, partial
models, conversions, JSON, cursor, wide keys). Add to the matching test
rather than creating a new file; each test opens its own in-memory
database through `setup()`. `crates/turso-orm/examples/relations.rs` is
the snippet source for the relations, entities and queries pages: keep
its `--8<--` markers when editing it.
