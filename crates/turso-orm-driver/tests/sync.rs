//! End-to-end check of an embedded replica against a live Turso Cloud database.
//!
//! The test needs credentials, so it is skipped unless `TURSO_DATABASE_URL`
//! and `TURSO_AUTH_TOKEN` are set; CI does not have them. It opens a replica
//! in a temporary directory and the same database over HTTP, writes on each
//! side and checks that `push` and `pull` carry the rows across, that `stats`
//! reflects the pushes and that `checkpoint` runs. Point it at a scratch
//! database: it creates and drops a table named after the process id.
//!
//! ```text
//! TURSO_DATABASE_URL=turso://… TURSO_AUTH_TOKEN=… \
//!     cargo test -p turso-orm-driver --features sync,serverless --test sync
//! ```

#![cfg(all(feature = "sync", feature = "serverless"))]

use turso_orm_driver::{ConnectOptions, ConnectionTrait, Database, Statement, TransactionTrait};

/// Reads the credentials from the environment, or `None` to skip.
fn credentials() -> Option<(String, String)> {
    let url = std::env::var("TURSO_DATABASE_URL").ok()?;
    let token = std::env::var("TURSO_AUTH_TOKEN").ok()?;
    Some((url, token))
}

/// The number of rows of `table` seen through `db`.
async fn count(db: &Database, table: &str) -> i64 {
    db.query_one(Statement::from_string(format!(
        "SELECT COUNT(*) AS n FROM {table}"
    )))
    .await
    .expect("count")
    .expect("row")
    .get("n")
    .expect("n")
}

/// A replica's writes reach the cloud on `push` and the cloud's writes reach the replica on
/// `pull`, which reports whether anything changed; `stats` counts the pending changes and
/// `checkpoint` folds the log.
#[tokio::test]
async fn replica_pushes_and_pulls() {
    let Some((url, token)) = credentials() else {
        eprintln!("TURSO_DATABASE_URL and TURSO_AUTH_TOKEN not set; skipping");
        return;
    };
    let dir = tempfile::tempdir().expect("tempdir");
    let replica = Database::connect(
        ConnectOptions::sync(dir.path().join("replica.db"), url.clone()).auth_token(token.clone()),
    )
    .await
    .expect("connect replica");
    let remote = Database::connect(ConnectOptions::remote(url).auth_token(token))
        .await
        .expect("connect remote");
    let table = format!("t_orm_sync_{}", std::process::id());

    // The schema and a first row are written locally; the cloud sees them
    // only after the push.
    replica
        .execute_unprepared(&format!(
            "CREATE TABLE {table} (id INTEGER PRIMARY KEY, name TEXT NOT NULL)"
        ))
        .await
        .expect("create");
    replica
        .execute_unprepared(&format!("INSERT INTO {table} (name) VALUES ('alice')"))
        .await
        .expect("insert locally");
    let before = replica.stats().await.expect("stats");
    assert!(before.pending_operations >= 1, "{before:?}");
    assert!(before.last_push_unix_time.is_none(), "{before:?}");
    replica.push().await.expect("push");
    assert_eq!(count(&remote, &table).await, 1);
    let after = replica.stats().await.expect("stats");
    assert_eq!(after.pending_operations, 0, "{after:?}");
    assert!(after.last_push_unix_time.is_some(), "{after:?}");
    assert!(after.network_sent_bytes > before.network_sent_bytes);
    replica.checkpoint().await.expect("checkpoint");

    // A row written over HTTP reaches the replica on the next pull, and a
    // second pull with nothing new says so.
    remote
        .execute_unprepared(&format!("INSERT INTO {table} (name) VALUES ('bob')"))
        .await
        .expect("insert remotely");
    assert!(replica.pull().await.expect("pull"));
    assert_eq!(count(&replica, &table).await, 2);
    assert!(!replica.pull().await.expect("second pull"));

    // A plain database has no replica to synchronise or to report on.
    assert!(remote.push().await.is_err());
    assert!(remote.stats().await.is_err());

    // The replica's engine refuses a second connection, a pragma and a write
    // while a transaction holds the write lock; the pool waits for the busy
    // timeout instead of surfacing that, so a writer next to an open
    // transaction succeeds once the transaction ends.
    let txn = replica.begin().await.expect("begin");
    txn.execute_unprepared(&format!("INSERT INTO {table} (name) VALUES ('carol')"))
        .await
        .expect("insert in txn");
    let writer = {
        let replica = replica.clone();
        let table = table.clone();
        tokio::spawn(async move {
            replica
                .execute_unprepared(&format!("INSERT INTO {table} (name) VALUES ('dave')"))
                .await
        })
    };
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    txn.commit().await.expect("commit");
    writer
        .await
        .expect("join")
        .expect("the writer waited for the lock");
    assert_eq!(count(&replica, &table).await, 4);

    // The rows are pushed before the table goes: a pull that brings a `DROP
    // TABLE` while the replica still holds unpushed rows of that table fails
    // inside the engine with a syntax error.
    replica.push().await.expect("push before the drop");
    remote
        .execute_unprepared(&format!("DROP TABLE {table}"))
        .await
        .expect("drop");
    replica.pull().await.expect("pull the drop");
}
