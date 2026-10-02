# Queries

Reading is done with a builder that starts from the entity and ends with a
call that runs the statement. This page explains how the builder is put
together, what SQL each method adds, how results come back, and how to go
beyond the typed API when you need to.

## The shape of a query

`Entity::find()` returns a `Select` typed by the entity. Every method on
it adds a clause and returns the builder, until one of the terminal
methods executes it:

| Terminal | SQL | Result |
|---|---|---|
| `one(db)` | adds `LIMIT 1` | `Option<Model>` |
| `all(db)` | as built | `Vec<Model>` |
| `count(db)` | wraps the query in `SELECT COUNT(*) FROM (...)` | `u64` |
| `exists(db)` | wraps the query in `SELECT EXISTS (...)` | `bool` |
| `stream(db)` | as built | a stream of `Model`s |
| `paginate(db, size)` | adds `LIMIT ... OFFSET ...` per page | a `Paginator` |
| `cursor_by([columns])` | adds a key boundary and `ORDER BY` per page | a `Cursor` |

The builder knows the entity's `Column` enum, so a filter on a column that
does not exist is a compile error. Every column reference is rendered
qualified, as `"cake"."price"`, which is what keeps a query unambiguous
once it joins another table that has a column of the same name.

## Finding rows

```rust
--8<-- "examples/basic/src/query.rs:filter"
```

`find_by_id` is the shortcut for a lookup by primary key; it takes a
value, or a tuple for a composite key. `filter` takes anything that
converts into a condition: a single comparison such as
`Column::Name.contains("chocolate")`, which renders
`"cake"."name" LIKE '%chocolate%'`, or a `Condition` that groups several.

Several `filter` calls are combined with `AND`. To build an `OR`, start
from `Condition::any()` and `add` each branch; `Condition::all()` is the
explicit `AND` form, and the two nest freely. The example above finds the
cakes that are gluten free *or* cost more than ten.

### The operators

`ColumnTrait` provides the comparison builders on every `Column`:

| Method | SQL |
|---|---|
| `eq`, `ne`, `gt`, `gte`, `lt`, `lte` | `=`, `<>`, `>`, `>=`, `<`, `<=` |
| `between(a, b)`, `not_between(a, b)` | `BETWEEN a AND b` |
| `like`, `not_like` | `LIKE`, with the pattern you give |
| `starts_with`, `ends_with`, `contains` | `LIKE 'x%' ESCAPE '\'` and friends: `%`, `_` and `\` in your text match themselves |
| `is_null`, `is_not_null` | `IS NULL`, `IS NOT NULL` |
| `is_in(iter)`, `is_not_in(iter)` | `IN (...)`, `NOT IN (...)` |
| `in_subquery(select)`, `not_in_subquery(select)` | `IN (SELECT ...)`, `NOT IN (SELECT ...)` |
| `eq_col(other)` | `= other.column`, to compare two columns across a join |
| `matches(query)` | Full-text `MATCH`, for columns with a Turso FTS index |
| `max`, `min`, `sum`, `avg`, `count` | The aggregate expression, for projections |

Values are bound as parameters, never interpolated, so a user-supplied
string in a filter is safe.

## Ordering, limiting, grouping

`order_by_asc(column)`, `order_by_desc(column)` and
`order_by(column, Order)` add `ORDER BY` clauses in the order you call
them. `limit(n)` and `offset(n)` map directly to SQL; `distinct()` adds
`DISTINCT`; `group_by(column)` and `having(condition)` are there for
aggregate queries. `count` deliberately drops ordering, limit and offset
from the inner query: they do not change the total and would only slow
it down.

## Pagination and streaming

Two tools cover large result sets. The paginator is for pages a user
browses; the stream is for processing every row without holding them all
in memory.

```rust
--8<-- "examples/basic/src/query.rs:paginate_stream"
```

`paginate(db, page_size)` returns a `Paginator` over the query. Pages are
counted from zero: `fetch_page(0)` is the first one. `num_items` runs a
`COUNT(*)`, `num_pages` divides it by the page size, and
`num_items_and_pages` does both in one call so that a listing endpoint
can answer with the total and the current page from one query.

`for_each_page` walks every page in turn and stops when a page comes back
short or when your closure returns `false`.

`stream(db)` executes the statement and yields models as the engine
produces them. The stream borrows a pooled connection for its whole life,
so drop it when you are done. It is `Unpin`, which is why the
`while let Some(x) = stream.try_next().await?` loop works without pinning;
`try_next` comes from `futures_util::TryStreamExt`.

### Keyset pagination

Offset pagination re-reads every row before the page and shifts when rows
are inserted ahead of it. For an API that hands out "the next page after
this one", a cursor is the better fit: it remembers the key of the last
row seen and asks for the rows after it, which is an index seek that
stays stable under concurrent writes.

```rust
--8<-- "crates/turso-orm/examples/relations.rs:cursor"
```

`cursor_by([columns])` names the key, most significant column first, so
`[Column::CreatedAt, Column::Id]` orders by date with the id as a
tie-breaker. `after(key)` and `before(key)` set the boundaries, with a
tuple for a composite key; `first(n)` takes the page from the start of
the range and `last(n)` from the end, both returned in key order; `desc()`
flips the direction. A composite key is compared lexicographically, with
the expanded `a > ? OR (a = ? AND b > ?)` form, so the statement only
uses operators every SQLite build accepts.

## Custom projections

Not every query returns table rows. To select a few columns or an
aggregate, switch the builder to a custom shape with `select_only`, add
columns and expressions, and decode into any struct that derives
`FromQueryResult`:

```rust
--8<-- "examples/basic/src/query.rs:project_struct"

--8<-- "examples/basic/src/query.rs:project"
```

`select_only` clears the entity's column list; `column`, `column_as` and
`expr_as(expression, alias)` add output columns; `into_model::<T>()` tells
the builder to decode rows into `T` by column name. The struct's field
names must match the aliases, or carry `#[turso(column_name = "...")]`
when they do not, as `most_expensive` does for `max_price`.
`Func::count_star()` and the aggregate methods on columns are the usual
ingredients.

Three other shapes avoid the struct altogether:

```rust
--8<-- "crates/turso-orm/examples/relations.rs:projections"
```

| Call | You get |
|---|---|
| `into_partial_model::<P>()` | A struct deriving `DerivePartialModel`, which selects its own columns; see [entities](entities.md#projections-that-are-not-entities). |
| `into_tuple::<(A, B)>()` | Tuples decoded by position in the select list, after `select_only`. |
| `into_json()` | `serde_json::Value` objects keyed by column name, behind the `with-json` feature. |
| `exists(db)` | Whether at least one row matches, without reading any. |

## Writing many rows at once

`insert`, `update` and `delete` on an active model work one row at a
time. The entity offers bulk forms:

```rust
--8<-- "examples/basic/src/mutation.rs:bulk_transaction"
```

| Call | SQL | Returns |
|---|---|---|
| `Entity::insert_many(iter).exec(db)` | One multi-row `INSERT` | The number of rows |
| `Entity::insert_many(iter).exec_with_returning(db)` | The same with `RETURNING *` | The stored models |
| `Entity::update_many().col(column, value).filter(...).exec(db)` | `UPDATE ... SET ... WHERE ...` | `rows_affected` |
| `Entity::update_many().col(...).filter(...).exec_with_returning(db)` | The same with `RETURNING *` | The changed models |
| `Entity::delete_many().filter(...).exec(db)` | `DELETE ... WHERE ...` | `rows_affected` |
| `Entity::delete_many().filter(...).exec_with_returning(db)` | The same with `RETURNING *` | The removed models |

A bulk insert takes any iterator of active models; each one contributes
its `Set` fields. The statement has one column list, the union of what
the models set, and SQLite cannot ask for the default of a single cell:
a model that leaves one of those columns unset contributes the default
the entity declares for it with `default_value`, or `NULL` when it
declares none. Without a `filter`, `update_many` and `delete_many` touch
the whole table, which is occasionally what you want and is worth a
second look otherwise. `Entity::delete(active_model)` has the same
`exec_with_returning`, for the row as it was just before removal.

Writing from a request body goes through `DeriveIntoActiveModel` or
`ActiveModel::from_json`; both are on the
[entities page](entities.md#from-a-request-to-an-active-model).

```rust
--8<-- "crates/turso-orm/examples/relations.rs:requests"
```

## Handling errors

Every call returns `Result<T, DbErr>`. Most variants describe a misuse
you can fix at development time; the ones a running application has to
handle come from the database itself and are classified by the driver, so
that a handler matches on structure rather than on an error message.

| Variant | Meaning |
|---|---|
| `DbErr::Driver(e)` | An engine or pool error. `e.kind()` tells busy from constraint from I/O; `e.constraint()` names the violated constraint kind. |
| `DbErr::RecordNotFound(what)` | A lookup that had to return a row returned none. |
| `DbErr::RecordNotInserted`, `RecordNotUpdated` | A write produced no row, for example under `ON CONFLICT DO NOTHING` or because the key matched nothing. |
| `DbErr::AttrNotSet(field)` | A `try_into_model` met a field that is `NotSet`. |
| `DbErr::PrimaryKeyNotSet` | An `update` or `delete` without a key. |
| `DbErr::Type(msg)`, `Json(msg)` | A value could not be decoded as the requested type. |
| `DbErr::Migration(msg)` | A migration refused to run or to revert. |
| `DbErr::Custom(msg)` | Reserved for application code and hooks. |

Three predicates on `DbErr` cover what a request handler usually needs,
and avoid digging into the driver error:

```rust
match post.insert(&db).await {
    Ok(model) => /* 201 Created */,
    Err(e) if e.is_unique_violation() => /* 409 Conflict */,
    Err(e) if e.is_foreign_key_violation() => /* 422 Unprocessable */,
    Err(e) if e.is_busy() => /* the lock did not clear in time: retry */,
    Err(e) => /* 500 */,
}
```

The [web examples](https://github.com/aartintelligent/turso-orm/tree/main/examples) map them to HTTP statuses in one place, in
the `api` crate of each.

## Beyond the typed builder

The typed API covers the common cases; it is not a wall.

- `Select::as_query()` and `query_mut()` expose the underlying
  `turso_sql::Select`, where any clause the SQL builder supports can be
  added, including Turso's vector functions.
- `Entity::find().from_raw_sql(statement)` runs a hand-written
  `Statement` and decodes its rows into the model;
  `Selector::<T>::from_statement(statement)` does the same for any
  `FromQueryResult` type.
- `db.execute`, `db.query_one` and `db.query_all` take a `Statement` and
  return raw `Row`s that decode on access with `row.get::<T>("column")`.

Whichever route you take, parameters stay bound and rows decode with the
same rules as the entity API.
