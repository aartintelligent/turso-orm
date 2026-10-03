# Transactions

A transaction groups several statements so that they either all take
effect or none does. This page explains how turso-orm binds a transaction
to a connection, what the four modes mean, how nesting works through
savepoints, what the closure form does for you, and what happens when a
transaction is dropped unfinished.

## Why a transaction pins a connection

In SQLite a transaction is a property of a connection: `BEGIN` on one
connection says nothing about another. A transaction handle must
therefore keep the connection it started on and send every statement
through it; a statement that went to a second connection would see a
different snapshot, or block on the first connection's lock.

That is what `db.begin()` does. It borrows one connection from the pool,
issues `BEGIN`, and returns a `Transaction` that holds that connection
until `commit` or `rollback`. The `Transaction` implements the same
`ConnectionTrait` as `Database`, so every query builder and every active
model method accepts `&txn` where it accepted `&db`:

```rust
let txn = db.begin().await?;
post::ActiveModel { /* ... */ }.insert(&txn).await?;
post::Entity::update_many()
    .col(post::Column::Views, 0)
    .exec(&txn)
    .await?;
txn.commit().await?;
```

Until `commit`, nothing written through `txn` is visible to statements
that go through `db`, because those run on other connections.

## Modes

`db.begin()` opens a `DEFERRED` transaction. `db.begin_with_mode(mode)`
picks another one:

| Mode | Statement | What it means |
|---|---|---|
| `Deferred` | `BEGIN DEFERRED` | No lock is taken until the first statement needs one. Reads take a shared lock; the first write upgrades to the write lock, which can fail with a busy error if another connection holds it. |
| `Immediate` | `BEGIN IMMEDIATE` | The write lock is taken right away. Use it when the transaction will write: the busy error, if any, happens at `BEGIN`, before any work is done. Migrations use this mode. |
| `Exclusive` | `BEGIN EXCLUSIVE` | Also blocks readers for the duration. Rarely needed. |
| `Concurrent` | `BEGIN CONCURRENT` | Turso's MVCC mode: several writers proceed on their own snapshot and conflicts surface at commit as busy errors. Requires `ConnectOptions::mvcc(true)`. |

When the database is locked, `BEGIN` does not fail at once: the driver
retries with a backoff for the duration of `busy_timeout`, five seconds
by default. Only when that budget is exhausted does the error reach you,
and `e.is_busy()` identifies it.

Over HTTP (`ConnectOptions::remote`), a transaction is a server-side
session that spans several requests. The four modes are sent as they are;
the busy timeout and MVCC are properties of a local engine and do not
apply.

## Nesting with savepoints

Calling `begin()` on a transaction, rather than on the database, opens a
nested transaction. SQLite has no nested `BEGIN`; the nested handle is a
`SAVEPOINT` on the same connection. Rolling it back undoes only the
statements made since the savepoint; committing it releases the
savepoint and leaves the outer transaction open, still uncommitted:

```rust
let txn = db.begin().await?;
tag::ActiveModel { /* ... */ }.insert(&txn).await?;     // kept
{
    let sp = txn.begin().await?;                         // SAVEPOINT sp1
    tag::ActiveModel { /* ... */ }.insert(&sp).await?;   // undone below
    sp.rollback().await?;                                // ROLLBACK TO sp1
}
txn.commit().await?;                                     // commits the first insert
```

This is how a service can attempt an optional step inside a larger unit
of work and discard just that step on failure. Savepoints nest to any
depth; `Transaction::depth()` tells how deep a handle is.

The engine keeps a single stack of savepoints, so only the innermost open
transaction may act. While `sp` is open, running a statement through
`txn`, beginning a second savepoint from it or committing it fails with
an `ErrorKind::Misuse` error: the statement would otherwise run inside
`sp` and share its fate. Finish or drop the nested transaction first. A
parent that fails this way on `commit` or `rollback` is rolled back, and
a nested handle whose top level has finished can no longer run anything.

## The closure form

Most transactions follow the same script: begin, run some statements,
commit if everything succeeded, roll back otherwise. `db.transaction`
does that script for you:

```rust
--8<-- "examples/basic/src/mutation.rs:bulk_transaction"
```

The closure receives `&Transaction` and returns a pinned, boxed future,
hence the `Box::pin(async move { ... })` wrapper. If the future resolves
to `Ok(value)`, the transaction is committed and `value` is returned; if
it resolves to `Err(e)`, the transaction is rolled back and `e` is
returned. The example above returns an error on purpose to show that the
insert made inside the closure is gone afterwards.

The error type is yours to choose, as long as it can be built from the
driver's error with `From`; `DbErr` qualifies, and so does an application
error enum that wraps it. That lets business validation inside the
closure abort the transaction with its own error.

## What happens on drop

`Drop` cannot await, so a transaction dropped without `commit` or
`rollback` cannot clean up synchronously. turso-orm makes that safe in
two ways:

- A dropped **top-level** transaction discards its connection instead of
  returning it to the pool. The engine rolls back whatever that connection
  held when it closes, and the pool opens a fresh connection on the next
  acquire. Nothing leaks into another borrower.
- A dropped **nested** transaction records its savepoint. The parent runs
  `ROLLBACK TO SAVEPOINT` before its next statement, so the abandoned
  work is undone before anything else happens on that connection.

These are safety nets, not the API: call `commit` or `rollback`
explicitly, or use the closure form, so that the outcome is decided where
the reader can see it.

## Reading under a transaction

Reads through `&txn` see the transaction's own writes and a consistent
snapshot of everything else. Reads through `&db` while a transaction is
open run on other connections and see the committed state only. When a
request handler must read what it just wrote, run both through the same
transaction.
