//! Writes: single inserts with and without `RETURNING`, a batch insert, an update and a delete.
//!
//! The raw side mirrors what the ORM asks the engine for: `insert/active_model` compares
//! `ActiveModel::insert`, which reads the stored row back with `RETURNING`, to the same
//! statement written by hand, and `insert/exec` compares `Entity::insert(...).exec`, which
//! only reports the generated key, to a plain insert and `last_insert_rowid`. The updates
//! follow the same split: `ActiveModel::update` reads the row back with `RETURNING`, the
//! `update_many` builder does not. Inserted rows stay in the table; nothing in this file
//! scans it, so the growth does not skew a group.

use std::hint::black_box;

use benches::{Fixture, RawPost, first, post, raw_execute, raw_query};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use turso::Value as RawValue;
use turso_orm::prelude::*;

/// Rows per batch insert.
const BATCH: usize = 100;
/// The raw insert that reads the row back.
const INSERT_RETURNING: &str =
    "INSERT INTO post (user_id, title, body) VALUES (?, ?, ?) RETURNING id, user_id, title, body";
/// The raw insert that only reports the key.
const INSERT: &str = "INSERT INTO post (user_id, title, body) VALUES (?, ?, ?)";
/// The raw update of one column by key that reads the row back.
const UPDATE_RETURNING: &str =
    "UPDATE post SET title = ? WHERE id = ? RETURNING id, user_id, title, body";
/// The raw update of one column by key.
const UPDATE: &str = "UPDATE post SET title = ? WHERE id = ?";
/// The raw delete by key.
const DELETE: &str = "DELETE FROM post WHERE id = ?";
/// The title every inserted or updated post gets.
const TITLE: &str = "Inserted by the benchmark";
/// The body every inserted post gets.
const BODY: &str = "A body of ordinary length, the same on every inserted row.";

/// A new post as the ORM inserts it: the key left unset for the engine to assign.
fn new_post(user_id: i32) -> post::ActiveModel {
    post::ActiveModel {
        user_id: Set(user_id),
        title: Set(TITLE.to_owned()),
        body: Set(BODY.to_owned()),
        ..Default::default()
    }
}

/// The bound values of a raw insert of one post.
fn new_post_values(user_id: i64) -> Vec<RawValue> {
    vec![
        RawValue::Integer(user_id),
        RawValue::Text(TITLE.to_owned()),
        RawValue::Text(BODY.to_owned()),
    ]
}

/// The raw multi-row insert of [`BATCH`] posts.
fn batch_sql() -> String {
    let rows = vec!["(?, ?, ?)"; BATCH].join(", ");
    format!("INSERT INTO post (user_id, title, body) VALUES {rows}")
}

/// One insert that returns the stored row.
fn active_model(c: &mut Criterion, fx: &Fixture) {
    let mut group = c.benchmark_group("insert/active_model");
    group.bench_function("orm", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let inserted = new_post(fx.user_id())
                .insert(&fx.orm)
                .await
                .expect("insert");
            black_box(inserted)
        });
    });
    group.bench_function("raw", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let values = new_post_values(i64::from(fx.user_id()));
            let rows = raw_query(&fx.raw, INSERT_RETURNING, values).await;
            black_box(first(rows, |row| RawPost::from_row(row, 0)).await)
        });
    });
    group.finish();
}

/// One insert that only reports the generated key.
fn exec(c: &mut Criterion, fx: &Fixture) {
    let mut group = c.benchmark_group("insert/exec");
    group.bench_function("orm", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let result = post::Entity::insert(new_post(fx.user_id()))
                .exec(&fx.orm)
                .await
                .expect("insert");
            black_box(result.last_insert_id)
        });
    });
    group.bench_function("raw", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let values = new_post_values(i64::from(fx.user_id()));
            raw_execute(&fx.raw, INSERT, values).await;
            black_box(fx.raw.last_insert_rowid())
        });
    });
    group.finish();
}

/// A batch of [`BATCH`] posts in one multi-row statement.
fn many(c: &mut Criterion, fx: &Fixture) {
    let sql = batch_sql();
    let mut group = c.benchmark_group("insert/many");
    group.throughput(Throughput::Elements(BATCH as u64));
    group.bench_function("orm", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let user_id = fx.user_id();
            let models = (0..BATCH).map(|_| new_post(user_id));
            let inserted = post::Entity::insert_many(models)
                .exec(&fx.orm)
                .await
                .expect("insert");
            black_box(inserted)
        });
    });
    group.bench_function("raw", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let user_id = i64::from(fx.user_id());
            let values: Vec<RawValue> = (0..BATCH).flat_map(|_| new_post_values(user_id)).collect();
            black_box(raw_execute(&fx.raw, &sql, values).await)
        });
    });
    group.finish();
}

/// The bound values of a raw update of one post's title.
fn update_values(id: i64) -> Vec<RawValue> {
    vec![RawValue::Text(TITLE.to_owned()), RawValue::Integer(id)]
}

/// One column of one row by key through an active model, which returns the stored row.
fn active_model_update(c: &mut Criterion, fx: &Fixture) {
    let mut group = c.benchmark_group("update/active_model");
    group.bench_function("orm", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let model = post::ActiveModel {
                id: Unchanged(fx.post_id()),
                title: Set(TITLE.to_owned()),
                ..Default::default()
            };
            black_box(model.update(&fx.orm).await.expect("update"))
        });
    });
    group.bench_function("raw", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let values = update_values(i64::from(fx.post_id()));
            let rows = raw_query(&fx.raw, UPDATE_RETURNING, values).await;
            black_box(first(rows, |row| RawPost::from_row(row, 0)).await)
        });
    });
    group.finish();
}

/// One column of one row by key through the bulk builder, which only reports the row count.
fn update_many(c: &mut Criterion, fx: &Fixture) {
    let mut group = c.benchmark_group("update/many");
    group.bench_function("orm", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let result = post::Entity::update_many()
                .col(post::Column::Title, TITLE)
                .filter(post::Column::Id.eq(fx.post_id()))
                .exec(&fx.orm)
                .await
                .expect("update");
            black_box(result)
        });
    });
    group.bench_function("raw", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let values = update_values(i64::from(fx.post_id()));
            black_box(raw_execute(&fx.raw, UPDATE, values).await)
        });
    });
    group.finish();
}

/// An insert followed by the delete of that row, so that the table keeps its size; the cost
/// of the insert is the `insert/exec` group of this file.
fn delete(c: &mut Criterion, fx: &Fixture) {
    let mut group = c.benchmark_group("delete/insert_then_delete");
    group.bench_function("orm", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let result = post::Entity::insert(new_post(fx.user_id()))
                .exec(&fx.orm)
                .await
                .expect("insert");
            let deleted = post::Entity::delete_by_id(result.last_insert_id)
                .exec(&fx.orm)
                .await
                .expect("delete");
            black_box(deleted)
        });
    });
    group.bench_function("raw", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let values = new_post_values(i64::from(fx.user_id()));
            raw_execute(&fx.raw, INSERT, values).await;
            let id = fx.raw.last_insert_rowid();
            black_box(raw_execute(&fx.raw, DELETE, [id]).await)
        });
    });
    group.finish();
}

/// Seeds once and runs every group of this file.
fn benches(c: &mut Criterion) {
    let fx = Fixture::load();
    active_model(c, &fx);
    exec(c, &fx);
    many(c, &fx);
    active_model_update(c, &fx);
    update_many(c, &fx);
    delete(c, &fx);
}

criterion_group!(insert, benches);
criterion_main!(insert);
