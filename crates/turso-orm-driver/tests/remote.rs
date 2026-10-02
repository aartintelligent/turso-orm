//! End-to-end check of the serverless engine against a live Turso Cloud database.
//!
//! The test needs credentials, so it is skipped unless `TURSO_DATABASE_URL`
//! and `TURSO_AUTH_TOKEN` are set; CI does not have them. Point it at a
//! scratch database: it creates and drops a table named after the process
//! id.
//!
//! ```text
//! TURSO_DATABASE_URL=libsql://… TURSO_AUTH_TOKEN=… \
//!     cargo test -p turso-orm-driver --features serverless --test remote
//! ```

#![cfg(feature = "serverless")]

use turso_orm_driver::{ConnectOptions, ConnectionTrait, Database, Statement, TransactionTrait};

/// Reads the credentials from the environment, or `None` to skip.
fn credentials() -> Option<(String, String)> {
    let url = std::env::var("TURSO_DATABASE_URL").ok()?;
    let token = std::env::var("TURSO_AUTH_TOKEN").ok()?;
    Some((url, token))
}

/// A remote database answers the same statements as a local one: DDL, writes with
/// `RETURNING`, a transaction that commits and one that rolls back.
#[tokio::test]
async fn remote_round_trip() {
    let Some((url, token)) = credentials() else {
        eprintln!("TURSO_DATABASE_URL and TURSO_AUTH_TOKEN not set; skipping");
        return;
    };
    let db = Database::connect(ConnectOptions::remote(url).auth_token(token))
        .await
        .expect("connect");
    let table = format!("t_orm_{}", std::process::id());
    db.execute_unprepared(&format!(
        "CREATE TABLE {table} (id INTEGER PRIMARY KEY, name TEXT NOT NULL)"
    ))
    .await
    .expect("create");

    let row = db
        .query_one(Statement::from_string(format!(
            "INSERT INTO {table} (name) VALUES ('alice') RETURNING id, name"
        )))
        .await
        .expect("insert")
        .expect("returning");
    assert_eq!(row.get::<String>("name").expect("name"), "alice");

    let txn = db.begin().await.expect("begin");
    txn.execute_unprepared(&format!("INSERT INTO {table} (name) VALUES ('bob')"))
        .await
        .expect("insert in txn");
    txn.rollback().await.expect("rollback");
    let count: i64 = db
        .query_one(Statement::from_string(format!(
            "SELECT COUNT(*) AS n FROM {table}"
        )))
        .await
        .expect("count")
        .expect("row")
        .get("n")
        .expect("n");
    assert_eq!(count, 1);

    db.execute_unprepared(&format!("DROP TABLE {table}"))
        .await
        .expect("drop");
}
