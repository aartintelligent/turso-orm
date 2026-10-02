# Entities

An entity is how turso-orm knows a table. This page explains what the
derive macro generates from a `Model` struct, how each attribute changes
the generated code and the DDL, how Rust types map to column types, and
how the active model decides what a write touches.

## One module per table

The convention is a module named after the table, holding a `Model`
struct and a `Relation` enum. The macros fill the module with the rest,
so that `cake::Entity`, `cake::Column`, `cake::ActiveModel` and
`cake::Model` all live side by side under one name:

```rust
--8<-- "examples/basic/src/entity/cake.rs"
```

What `DeriveEntityModel` produces from that struct:

| Generated item | What it is for |
|---|---|
| `Entity` | A unit struct that starts queries: `Entity::find()`, `Entity::insert_many(...)`, `Entity::delete_by_id(...)`. It also knows the table name and the columns. |
| `Column` | An enum with one variant per field, in `UpperCamelCase`. It is what you pass to `filter`, `order_by` and the loaders, and it carries the condition builders (`eq`, `gt`, `contains`, ...). |
| `PrimaryKey` | An enum of the key columns, used by `find_by_id`, `update` and `delete`. |
| `ActiveModel` | The write-side twin of `Model`, described [below](#the-active-model). |

`Model` itself stays exactly as you wrote it. It is a plain value: the
ORM decodes rows into it and hands it back to you, and you can derive
`Serialize` or anything else on it, as the web examples do.

!!! warning "Do not derive `PartialEq` on `Column`"
    The macro deliberately leaves `PartialEq` off the generated `Column`
    enum. `Column::Name.eq("x")` is the condition builder; if the enum
    implemented `PartialEq`, Rust would resolve that call to the trait
    method and the filter would silently become a boolean.

## Attributes

Everything is declared with `#[turso(...)]`. On the struct, `table_name`
is the only attribute and it is required: the macro does not guess a
table name from the struct name.

On a field:

| Attribute | Effect |
|---|---|
| `primary_key` | The field is part of the primary key. A single integer key becomes `INTEGER PRIMARY KEY AUTOINCREMENT`, which is SQLite's row id, so the database assigns it. Several `primary_key` fields form a composite key, declared at table level, with no auto-increment. |
| `auto_increment` | Forces `AUTOINCREMENT` on, for a single integer key that would not get it by default. |
| `column_name = "..."` | The column name, when it must differ from the field name turned into `snake_case`. |
| `unique` | Adds a `UNIQUE` constraint to the column in the generated DDL. |
| `indexed` | Makes `Schema::create_index_from_entity` emit a `CREATE INDEX` for the column. Foreign-key columns are a typical candidate, since SQLite does not index them on its own. |
| `nullable` | Accepted for an explicit declaration, but nullability is already implied by `Option<T>`. |
| `default_value = "..."` | A literal `DEFAULT` in the DDL. It is inlined as text because SQLite does not bind parameters in DDL. |
| `ignore` | The field is not a column. It must implement `Default`, since the ORM fills it when decoding a row, and it is left out of the active model. |

The relation attributes (`has_many`, `has_one`, `belongs_to`, `from`,
`to`, `via`, `on_delete`, `on_update`, `skip_fk`) go on the `Relation`
enum and are explained on the [relations page](relations.md).

## Column types

The macro asks each field type how it should be declared, through the
`TursoType` trait. Wrapping a type in `Option` makes the column nullable
and changes nothing else.

| Rust type | Column type | Stored as |
|---|---|---|
| `bool` | `Boolean` | `INTEGER`, `0` or `1` |
| `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32` | `Integer` | `INTEGER` |
| `f32`, `f64` | `Real` | `REAL` |
| `String` | `Text` | `TEXT` |
| `Vec<u8>` | `Blob` | `BLOB` |
| `NaiveDate`, `NaiveTime`, `NaiveDateTime` | `Date`, `Time`, `DateTime` | `TEXT`, ISO 8601 |
| `DateTime<Utc>`, `DateTime<FixedOffset>` | `TimestampWithTimeZone` | `TEXT`, RFC 3339 |
| `Uuid` | `Uuid` | `TEXT`, hyphenated |
| `serde_json::Value` | `Json` | `TEXT` |
| `Decimal` | `Decimal` | `TEXT` |

An enum of your own can be a column type too; see
[enum columns](#enum-columns) below.

The right column matters because SQLite has only five storage classes.
Declaring a column as `DATETIME` is a hint for readers; what the engine
stores is text, and what turso-orm writes in the DDL are the storage
classes themselves, so that tables can be created as `STRICT` without
surprises.

### How values decode

Decoding is driven by the Rust type you ask for, not by the column's
declared type, and it is lenient in the way SQLite users expect: an
integer decodes into a `bool`, an integral `REAL` into an integer, a
number into a `String`, text into a date, a UUID or JSON. That keeps rows
written by other tools readable.

There is one strict rule. `Option<T>` is the only type that accepts
`NULL`. Reading a nullable column into a plain `String` fails with a
`DbErr::Type` that names the column, instead of defaulting to an empty
string. A model field that can be `NULL` must therefore be an `Option`,
and that is also what makes the generated column nullable.

### Enum columns

SQLite has no enum type, so an enum column is stored as the text or the
integer each variant maps to. `DeriveActiveEnum` records that mapping on
a fieldless enum and makes it a column type in every sense: a model
field, a value in a condition, a default in the DDL.

```rust
--8<-- "crates/turso-orm/examples/relations.rs:active_enum"
```

| Attribute | Effect |
|---|---|
| `rs_type = "String"` on the enum | Stored as `TEXT`; a variant without `string_value` is stored as its `snake_case` name. |
| `rs_type = "i32"` (or another integer type) on the enum | Stored as `INTEGER`; every variant needs a `num_value`. |
| `string_value = "..."`, `num_value = n` on a variant | The stored value. |

A stored value no variant maps to is a decoding error that names the
column, never a silent default. `Status::values()` lists the variants,
and `Column::Status.eq(Status::Published)` binds the stored value like
any other.

## The active model

A `Model` is a snapshot of a row. To write, you need to say which fields
should go to the database, and that is the job of the `ActiveModel`: the
same fields, each wrapped in an `ActiveValue<T>` with three states.

| State | Meaning | On `insert` | On `update` |
|---|---|---|---|
| `Set(v)` | Write this value | Included in the `INSERT` | Included in the `SET` |
| `Unchanged(v)` | Known, not to be written | Included, since the row does not exist yet | Left alone |
| `NotSet` | Unknown | Omitted, so the database default applies | Left alone |

`ActiveModel` implements `Default` with every field `NotSet`, which is
why `..Default::default()` is the idiom for "only these fields":

```rust
--8<-- "examples/basic/src/mutation.rs:insert_update"
```

The first insert sends only `name` and `profit_margin`; `id` is `NotSet`,
so SQLite assigns a row id, and `RETURNING *` brings the full row back as
a `Model`. The update shows the other direction: converting a `Model` into
an `ActiveModel` with `.into()` marks every field `Unchanged`, so after
`cake.price = Set(11.0)` the `UPDATE` statement contains that single
column and a `WHERE` on the primary key.

### Insert, update, save, delete

| Method | What it does | Returns |
|---|---|---|
| `insert(db)` | `INSERT ... RETURNING *` with the `Set` and `Unchanged` fields. | The stored `Model`. |
| `update(db)` | `UPDATE ... SET <Set fields> WHERE <key> RETURNING *`. With no `Set` field it simply fetches the row. Fails with `PrimaryKeyNotSet` if a key field is `NotSet`, and with `RecordNotUpdated` if no row matches. | The stored `Model`. |
| `save(db)` | `insert` when the key is `NotSet` or `Set`, `update` when it is `Unchanged`. | The `ActiveModel`, every field `Unchanged`, ready to be modified and saved again. |
| `delete(db)` | `DELETE ... WHERE <key>`. | A `DeleteResult` with `rows_affected`. |

`save` is the convenient one for code that does not care whether the row
exists yet:

```rust
--8<-- "examples/basic/src/mutation.rs:save_delete"
```

The first `save` inserts because `id` is `NotSet`. It returns an active
model whose `id` is now `Unchanged(1)`, so the second `save`, after
changing `name`, runs an `UPDATE` of that one column.

### Hooks

`ActiveModelBehavior` is the trait you implement, usually empty, on every
`ActiveModel`. Its four methods have default no-op bodies and let you
step in around writes:

| Hook | When it runs | Typical use |
|---|---|---|
| `before_save(self, db, insert)` | Before `insert` (`insert == true`) or `update` | Normalise a field, fill a timestamp, validate and return an error |
| `after_save(model, db, insert)` | After the row is stored | Audit, cache invalidation |
| `before_delete(self, db)` | Before `delete` | Refuse the deletion of a protected row |
| `after_delete(self, db)` | After `delete` | Clean up related resources |

A hook that returns `Err` aborts the write; the error reaches the caller
unchanged.

### From a request to an active model

A request body rarely matches the model one for one: it carries a subset
of the columns and leaves the rest to defaults. `DeriveIntoActiveModel`
turns such a struct into the entity's active model field by field:

```rust
--8<-- "crates/turso-orm/examples/relations.rs:into_active_model"
```

A plain field becomes `Set`. A field wrapped in one more `Option` than the
column type becomes `Set` when `Some` and `NotSet` when `None`, so an
optional field of the request leaves the column alone and the database
default applies; for a nullable column, that outer `Option` is
`Option<Option<T>>`. `#[turso(ignore)]` leaves a field out, and
`#[turso(active_model = "path::ActiveModel")]` names the target when it
is not the `ActiveModel` in scope.

The same conversion exists from JSON, behind the `with-json` feature:
`ActiveModel::from_json(value)` sets every attribute whose column name is
a key of the object and ignores the other keys, and `set_from_json` does
it on an existing active model. Numbers, strings, booleans and `null`
map onto the storage classes; an array or an object is stored as its
JSON text, which is what a JSON column expects.

The reverse direction is `try_into_model()`: an active model whose every
attribute carries a value, `Set` or `Unchanged`, becomes the plain
`Model`; the first `NotSet` attribute makes it fail with
`DbErr::AttrNotSet` naming the column. A `Model` also has `delete(db)`,
which runs through the active model and its hooks.

## Composite keys

Several `primary_key` fields form a composite key, up to six columns.
Nothing else changes in the model, but a few call sites take a tuple
instead of a single value: `find_by_id((7, "rust".to_owned()))`, and the
same shape for `delete_by_id`. Composite keys are never auto-incremented,
so every key field must be `Set` on insert.

## Projections that are not entities

Sometimes a query returns a shape that is not a table row: an aggregate,
a join of two tables, a handful of columns. Two derives cover it.

A plain struct can derive `FromQueryResult` and be decoded by column
name, with `#[turso(column_name = "...")]` to rename a field; you build
the select list yourself. The
[queries page](queries.md#custom-projections) shows one.

A **partial model** goes one step further and knows which columns to
select. Derive `DerivePartialModel`, name the entity, and each field
reads the column of the same name, the one given with `from_col`, or an
expression given with `from_expr`:

```rust
--8<-- "crates/turso-orm/examples/relations.rs:partial_model"
```

`Entity::find().into_partial_model::<Headline>()` replaces the select
list with the fields of the struct and decodes rows into it, so the
projection is declared once and cannot drift from its decoder.
