//! End-to-end check of an embedded replica against a live Turso Cloud database.
//!
//! The test needs credentials, so it is skipped unless `TURSO_DATABASE_URL`
//! and `TURSO_AUTH_TOKEN` are set; CI does not have them. It opens a replica
//! in a temporary directory and the same database over HTTP, writes on each
//! side and checks that `push` and `pull` carry the rows across. Point it at
//! a scratch database: it creates and drops a table named after the process
//! id.
//!
//! ```text
//! TURSO_DATABASE_URL=libsql://… TURSO_AUTH_TOKEN=… \
//!     cargo test -p turso-orm-driver --features sync,serverless --test sync
//! ```

#![cfg(all(feature = "sync", feature = "serverless"))]

use turso_orm_driver::{ConnectOptions, ConnectionTrait, Database, Statement};

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
/// `pull`, which reports whether anything changed.
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
    replica.push().await.expect("push");
    assert_eq!(count(&remote, &table).await, 1);

    // A row written over HTTP reaches the replica on the next pull, and a
    // second pull with nothing new says so.
    remote
        .execute_unprepared(&format!("INSERT INTO {table} (name) VALUES ('bob')"))
        .await
        .expect("insert remotely");
    assert!(replica.pull().await.expect("pull"));
    assert_eq!(count(&replica, &table).await, 2);
    assert!(!replica.pull().await.expect("second pull"));

    // A plain database has no replica to synchronise.
    assert!(remote.push().await.is_err());

    remote
        .execute_unprepared(&format!("DROP TABLE {table}"))
        .await
        .expect("drop");
    replica.pull().await.expect("pull the drop");
}
