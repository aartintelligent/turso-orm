//! End-to-end tests of the driver against a real Turso database.
//!
//! This is the integration tier: every test opens a real engine — in memory
//! or in a temporary file — through the public API and exercises the pool,
//! the transaction lifecycle, streaming and typed decoding together, which
//! the unit tests inside the crate cannot do. Nothing here is mocked, so a
//! failure points at the driver's contract with the engine rather than at
//! a test double.
//!
//! Run with:
//!
//! ```text
//! cargo test -p turso-orm-driver --test driver
//! ```

use futures_util::TryStreamExt;
use turso_orm_driver::{
    ConnectOptions, ConnectionTrait, ConstraintKind, Database, StreamTrait, Transaction,
    TransactionMode, TransactionTrait,
};
use turso_sql::prelude::*;

/// Opens a small in-memory pool with one `item` table.
///
/// The pool is capped at four connections so that the concurrency test
/// below has to recycle connections rather than open one per task.
async fn setup() -> Database {
    let db = Database::connect(ConnectOptions::in_memory().max_connections(4))
        .await
        .expect("open");
    db.execute_unprepared(
        "CREATE TABLE item (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE, price REAL, active INTEGER NOT NULL DEFAULT 1)",
    )
    .await
    .expect("create");
    db
}

/// Builds an insert of one item with the given name and optional price.
fn insert(name: &str, price: Option<f64>) -> Statement {
    Query::insert()
        .into_table("item")
        .columns(["name", "price"])
        .values([Expr::val(name), Expr::val(price)])
        .to_statement()
}

/// Inserts, updates and reads back a row, checking that each column can be
/// decoded by name or position into the type the caller asks for.
#[tokio::test]
async fn crud_and_typed_decoding() {
    let db = setup().await;
    let res = db.execute(insert("pen", Some(1.5))).await.expect("insert");
    assert_eq!(res.rows_affected, 1);
    assert_eq!(res.last_insert_id, 1);

    // The same row is read as several Rust types: lookup is by name or
    // position and case-insensitive, and a missing column is an error for
    // `get` but `None` for `try_get`.
    let row = db
        .query_one(Query::select().from("item").to_statement())
        .await
        .expect("query")
        .expect("row");
    assert_eq!(row.columns(), ["id", "name", "price", "active"]);
    assert_eq!(row.get::<i32>("id").unwrap(), 1);
    assert_eq!(row.get::<i64>(0).unwrap(), 1);
    assert_eq!(row.get::<String>("name").unwrap(), "pen");
    assert_eq!(row.get::<Option<f64>>("price").unwrap(), Some(1.5));
    assert!(row.get::<bool>("active").unwrap());
    assert_eq!(row.get::<String>("ACTIVE").unwrap(), "1");
    assert!(row.get::<i32>("missing").is_err());
    assert_eq!(row.try_get::<i32>("missing").unwrap(), None);

    // Setting the price to NULL shows that only `Option<T>` accepts it.
    let updated = db
        .execute(
            Query::update()
                .table("item")
                .value("price", Expr::val(Option::<f64>::None))
                .and_where(Expr::col("id").eq(1))
                .to_statement(),
        )
        .await
        .expect("update");
    assert_eq!(updated.rows_affected, 1);
    let rows = db
        .query_all(Query::select().column("price").from("item").to_statement())
        .await
        .expect("query");
    assert_eq!(rows[0].get::<Option<f64>>("price").unwrap(), None);
    assert!(rows[0].get::<f64>("price").is_err());
}

/// A unique-constraint violation is classified as such and is not mistaken
/// for lock contention.
#[tokio::test]
async fn constraint_errors_are_classified() {
    let db = setup().await;
    db.execute(insert("dup", None)).await.expect("first");
    let err = db
        .execute(insert("dup", None))
        .await
        .expect_err("duplicate");
    assert_eq!(err.constraint(), Some(ConstraintKind::Unique));
    assert!(!err.is_busy());
}

/// Every pooled connection sees the same in-memory database, and more
/// concurrent tasks than pool slots are served by recycling connections.
#[tokio::test]
async fn pool_shares_memory_database_across_tasks() {
    let db = setup().await;
    let handles: Vec<_> = (0..16)
        .map(|i| {
            let db = db.clone();
            tokio::spawn(async move { db.execute(insert(&format!("n{i}"), None)).await })
        })
        .collect();
    for h in handles {
        h.await.expect("join").expect("insert");
    }
    let row = db
        .query_one(Statement::from_string("SELECT COUNT(*) AS n FROM item"))
        .await
        .expect("count")
        .expect("row");
    assert_eq!(row.get::<i64>("n").unwrap(), 16);
}

/// Counts the items through any connection, so the same check runs on the
/// pool and inside a transaction.
async fn count(c: &impl ConnectionTrait) -> i64 {
    c.query_one(Statement::from_string("SELECT COUNT(*) AS n FROM item"))
        .await
        .expect("count")
        .expect("row")
        .get("n")
        .expect("n")
}

/// Savepoints commit and roll back independently, a dropped savepoint is
/// rolled back lazily, a dropped top-level transaction commits nothing, and
/// the closure form commits on `Ok`.
#[tokio::test]
async fn transactions_savepoints_and_drop() {
    let db = setup().await;
    let txn = db.begin().await.expect("begin");
    txn.execute(insert("a", None)).await.expect("a");
    // A rolled-back savepoint discards its own work only.
    {
        let sp: Transaction = txn.begin().await.expect("savepoint");
        assert_eq!(sp.depth(), 1);
        sp.execute(insert("b", None)).await.expect("b");
        sp.rollback().await.expect("rollback sp");
    }
    // A released savepoint keeps its work in the parent.
    {
        let sp = txn.begin().await.expect("savepoint");
        sp.execute(insert("c", None)).await.expect("c");
        sp.commit().await.expect("release sp");
    }
    // A savepoint dropped without commit is rolled back before the
    // parent's next statement, which here is the count.
    {
        let sp = txn.begin().await.expect("savepoint");
        sp.execute(insert("dropped", None)).await.expect("d");
    }
    assert_eq!(count(&txn).await, 2);
    txn.commit().await.expect("commit");
    assert_eq!(count(&db).await, 2);

    // A top-level transaction dropped without commit discards its
    // connection; nothing it wrote becomes visible.
    {
        let txn = db
            .begin_with_mode(TransactionMode::Immediate)
            .await
            .expect("begin");
        txn.execute(insert("ghost", None)).await.expect("ghost");
    }
    assert_eq!(count(&db).await, 2);

    // The closure form commits when the callback returns `Ok`.
    let n: i64 = db
        .transaction(|txn| {
            Box::pin(async move {
                txn.execute(insert("closure", None)).await?;
                Ok::<_, turso_orm_driver::Error>(count(txn).await)
            })
        })
        .await
        .expect("closure");
    assert_eq!(n, 3);
}

/// Rows stream in order from the pool, where the stream carries its own
/// pooled connection, and from a transaction, where it borrows the pinned
/// one.
#[tokio::test]
async fn streaming() {
    let db = setup().await;
    for i in 0..5 {
        db.execute(insert(&format!("s{i}"), None))
            .await
            .expect("insert");
    }
    let names: Vec<String> = db
        .stream(
            Query::select()
                .column("name")
                .from("item")
                .order_by("id", Order::Asc)
                .to_statement(),
        )
        .await
        .expect("stream")
        .map_ok(|row| row.get::<String>("name").unwrap())
        .try_collect()
        .await
        .expect("collect");
    assert_eq!(names, ["s0", "s1", "s2", "s3", "s4"]);

    let txn = db.begin().await.expect("begin");
    let rows: Vec<_> = txn
        .stream(Statement::from_string("SELECT id FROM item"))
        .await
        .expect("stream")
        .try_collect()
        .await
        .expect("collect");
    assert_eq!(rows.len(), 5);
    txn.commit().await.expect("commit");
}

/// A file-backed database is created on disk, answers a ping and runs a
/// multi-statement batch.
#[tokio::test]
async fn file_database_and_ping() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("t.db");
    let db = Database::connect(ConnectOptions::new(&path))
        .await
        .expect("open");
    db.ping().await.expect("ping");
    db.execute_unprepared("CREATE TABLE t (id INTEGER PRIMARY KEY); INSERT INTO t DEFAULT VALUES;")
        .await
        .expect("batch");
    assert!(path.exists());
}
