//! Reads: a point select, a filtered page, a full scan, a count and a keyset page.
//!
//! Each group runs the ORM call and the raw client doing the same work, decoded into the same
//! shape of Rust values, so that the difference is the cost of building the statement,
//! executing it through the pool and decoding the rows into models.

use std::hint::black_box;

use benches::{Fixture, POSTS, RawPost, collect, first, integer, post, raw_query};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use turso_orm::prelude::*;

/// Rows of a filtered page and of a keyset page.
const PAGE: u64 = 50;
/// The raw point select.
const BY_ID: &str = "SELECT id, user_id, title, body FROM post WHERE id = ?";
/// The raw filtered page.
const BY_USER: &str =
    "SELECT id, user_id, title, body FROM post WHERE user_id = ? ORDER BY id ASC LIMIT 50";
/// The raw full scan.
const SCAN: &str = "SELECT id, user_id, title, body FROM post";
/// The raw count.
const COUNT: &str = "SELECT COUNT(*) FROM post";
/// The raw keyset page.
const AFTER: &str =
    "SELECT id, user_id, title, body FROM post WHERE id > ? ORDER BY id ASC LIMIT 50";

/// One post by primary key, into a model.
fn by_id(c: &mut Criterion, fx: &Fixture) {
    let mut group = c.benchmark_group("select/by_id");
    group.bench_function("orm", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let id = fx.post_id();
            let found = post::Entity::find_by_id(id)
                .one(&fx.orm)
                .await
                .expect("select");
            black_box(found)
        });
    });
    group.bench_function("raw", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let id = i64::from(fx.post_id());
            let rows = raw_query(&fx.raw, BY_ID, [id]).await;
            black_box(first(rows, |row| RawPost::from_row(row, 0)).await)
        });
    });
    group.finish();
}

/// The posts of one user through the index, ordered and limited.
fn by_user(c: &mut Criterion, fx: &Fixture) {
    let mut group = c.benchmark_group("select/by_user");
    group.bench_function("orm", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let user_id = fx.user_id();
            let posts = post::Entity::find()
                .filter(post::Column::UserId.eq(user_id))
                .order_by_asc(post::Column::Id)
                .limit(PAGE)
                .all(&fx.orm)
                .await
                .expect("select");
            black_box(posts)
        });
    });
    group.bench_function("raw", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let user_id = i64::from(fx.user_id());
            let rows = raw_query(&fx.raw, BY_USER, [user_id]).await;
            black_box(collect(rows, |row| RawPost::from_row(row, 0)).await)
        });
    });
    group.finish();
}

/// Every post of the table into a `Vec` of models.
fn scan(c: &mut Criterion, fx: &Fixture) {
    let mut group = c.benchmark_group("select/scan");
    group.throughput(Throughput::Elements(u64::from(POSTS)));
    group.sample_size(20);
    group.bench_function("orm", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let posts = post::Entity::find().all(&fx.orm).await.expect("select");
            black_box(posts)
        });
    });
    group.bench_function("raw", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let rows = raw_query(&fx.raw, SCAN, ()).await;
            black_box(collect(rows, |row| RawPost::from_row(row, 0)).await)
        });
    });
    group.finish();
}

/// A `COUNT(*)` over the table.
fn count(c: &mut Criterion, fx: &Fixture) {
    let mut group = c.benchmark_group("select/count");
    group.bench_function("orm", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let n = post::Entity::find().count(&fx.orm).await.expect("count");
            black_box(n)
        });
    });
    group.bench_function("raw", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let rows = raw_query(&fx.raw, COUNT, ()).await;
            black_box(first(rows, |row| integer(row, 0)).await)
        });
    });
    group.finish();
}

/// A keyset page: the rows after a key, in key order.
fn cursor(c: &mut Criterion, fx: &Fixture) {
    let mut group = c.benchmark_group("select/cursor");
    group.bench_function("orm", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let after = fx.post_id();
            let page = post::Entity::find()
                .cursor_by([post::Column::Id])
                .after(after)
                .first(PAGE)
                .all(&fx.orm)
                .await
                .expect("select");
            black_box(page)
        });
    });
    group.bench_function("raw", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let after = i64::from(fx.post_id());
            let rows = raw_query(&fx.raw, AFTER, [after]).await;
            black_box(collect(rows, |row| RawPost::from_row(row, 0)).await)
        });
    });
    group.finish();
}

/// Seeds once and runs every group of this file.
fn benches(c: &mut Criterion) {
    let fx = Fixture::load();
    by_id(c, &fx);
    by_user(c, &fx);
    scan(c, &fx);
    count(c, &fx);
    cursor(c, &fx);
}

criterion_group!(select, benches);
criterion_main!(select);
