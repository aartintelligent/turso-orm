//! Rendering: the SQL text and bound values of a statement, with no engine involved.
//!
//! Each group builds the same statement twice: through the entity API of `turso-orm`, which is
//! what every query of the ORM does before it reaches the pool, and through the bare builder
//! of `turso-sql`. The difference is the cost of the typed layer; the `sql` side alone is the
//! cost of the writer. Nothing here allocates a database, so the numbers are stable enough to
//! catch a regression of a few percent.

use std::hint::black_box;

use benches::{post, user};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use turso_orm::prelude::*;
use turso_orm::sql::Query;

/// Rows of the batch insert.
const BATCH: usize = 100;
/// The columns of `post`, as the bare builder names them.
const POST_COLUMNS: [(&str, &str); 4] = [
    ("post", "id"),
    ("post", "user_id"),
    ("post", "title"),
    ("post", "body"),
];

/// A point select by primary key.
fn select_by_id(c: &mut Criterion) {
    let mut group = c.benchmark_group("render/select_by_id");
    group.bench_function("orm", |b| {
        b.iter(|| black_box(post::Entity::find_by_id(black_box(42)).build()));
    });
    group.bench_function("sql", |b| {
        b.iter(|| {
            let select = Query::select()
                .columns(POST_COLUMNS)
                .from("post")
                .and_where(Expr::col(("post", "id")).eq(black_box(42)));
            black_box(select.to_statement())
        });
    });
    group.finish();
}

/// A select with a join, a filter on the joined table, an order and a limit.
fn select_join(c: &mut Criterion) {
    let mut group = c.benchmark_group("render/select_join");
    group.bench_function("orm", |b| {
        b.iter(|| {
            let select = post::Entity::find()
                .inner_join(user::Entity)
                .filter(user::Column::Email.eq(black_box("user1@example.com")))
                .order_by_asc(post::Column::Id)
                .limit(10);
            black_box(select.build())
        });
    });
    group.bench_function("sql", |b| {
        b.iter(|| {
            let select = Query::select()
                .columns(POST_COLUMNS)
                .from("post")
                .inner_join(
                    "user",
                    Expr::col(("user", "id")).eq(Expr::col(("post", "user_id"))),
                )
                .and_where(Expr::col(("user", "email")).eq(black_box("user1@example.com")))
                .order_by(("post", "id"), Order::Asc)
                .limit(10);
            black_box(select.to_statement())
        });
    });
    group.finish();
}

/// A single-row insert from an active model with the key left unset.
fn insert_one(c: &mut Criterion) {
    let mut group = c.benchmark_group("render/insert_one");
    group.bench_function("orm", |b| {
        b.iter(|| {
            let model = post::ActiveModel {
                user_id: Set(black_box(1)),
                title: Set("Post 1".to_owned()),
                body: Set("A body".to_owned()),
                ..Default::default()
            };
            black_box(post::Entity::insert(model).build())
        });
    });
    group.bench_function("sql", |b| {
        b.iter(|| {
            let insert = Query::insert()
                .into_table("post")
                .columns(["user_id", "title", "body"])
                .values([
                    Expr::val(black_box(1)),
                    Expr::val("Post 1"),
                    Expr::val("A body"),
                ]);
            black_box(insert.to_statement())
        });
    });
    group.finish();
}

/// A multi-row insert of [`BATCH`] posts through the bare builder: the writer alone, since the
/// ORM's batch insert renders only on execution.
fn insert_many(c: &mut Criterion) {
    let mut group = c.benchmark_group("render/insert_many");
    group.throughput(Throughput::Elements(BATCH as u64));
    group.bench_function("sql", |b| {
        b.iter(|| {
            let mut insert = Query::insert()
                .into_table("post")
                .columns(["user_id", "title", "body"]);
            for i in 0..BATCH {
                insert = insert.values([
                    Expr::val(black_box(1)),
                    Expr::val(format!("Post {i}")),
                    Expr::val("A body"),
                ]);
            }
            black_box(insert.to_statement())
        });
    });
    group.finish();
}

/// The `CREATE TABLE` of an entity, as `Schema` renders it.
fn create_table(c: &mut Criterion) {
    c.bench_function("render/create_table", |b| {
        b.iter(|| {
            let table = Schema::new().create_table_from_entity(post::Entity);
            black_box(table.to_statement())
        });
    });
}

criterion_group!(
    render,
    select_by_id,
    select_join,
    insert_one,
    insert_many,
    create_table
);
criterion_main!(render);
