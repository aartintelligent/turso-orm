//! Relations: a join into pairs, a join grouped by the left side, the many-to-many loader and
//! a three-hop chain.
//!
//! The raw side writes the join by hand and groups the rows the way the ORM does, so that both
//! sides hand back the same nested values. The models the loader and the chain start from are
//! read once, before the measurement, since fetching them is the `select` file's business.

use std::hint::black_box;

use benches::{
    Fixture, POSTS_PER_USER, RawPost, RawTag, RawUser, USERS, UserToTag, collect, integer, post,
    raw_query, tag, user,
};
use criterion::{Criterion, criterion_group, criterion_main};
use turso::Value as RawValue;
use turso_orm::prelude::*;

/// The raw join of the posts of one user with their author.
const POSTS_WITH_USER: &str = "SELECT p.id, p.user_id, p.title, p.body, u.id, u.email, u.name \
    FROM post AS p LEFT JOIN user AS u ON u.id = p.user_id WHERE p.user_id = ?";
/// The raw join of one user with their posts.
const USER_WITH_POSTS: &str = "SELECT u.id, u.email, u.name, p.id, p.user_id, p.title, p.body \
    FROM user AS u LEFT JOIN post AS p ON p.user_id = u.id WHERE u.id = ? ORDER BY u.id";
/// The raw tags of up to ten posts through the junction, with the post each row belongs to.
const TAGS_OF_POSTS: &str = "SELECT pt.post_id, t.id, t.label FROM post_tag AS pt \
    INNER JOIN tag AS t ON t.id = pt.tag_id WHERE pt.post_id IN (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";
/// The raw three-hop chain from a user to the distinct tags of their posts.
const TAGS_OF_USER: &str = "SELECT DISTINCT t.id, t.label FROM tag AS t \
    INNER JOIN post_tag AS pt ON pt.tag_id = t.id INNER JOIN post AS p ON p.id = pt.post_id \
    WHERE p.user_id = ?";

/// The position of a seeded id in a `Vec` of models ordered by id.
fn index(id: i32) -> usize {
    usize::try_from(id - 1).expect("seeded ids start at 1")
}

/// The posts of one user, consecutive in the seeded order.
fn posts_of(posts: &[post::Model], user_id: i32) -> &[post::Model] {
    let per_user = usize::try_from(POSTS_PER_USER).expect("fits");
    let start = index(user_id) * per_user;
    &posts[start..start + per_user]
}

/// Posts of one user paired with their author.
fn find_also_related(c: &mut Criterion, fx: &Fixture) {
    let mut group = c.benchmark_group("relations/find_also_related");
    group.bench_function("orm", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let user_id = fx.user_id();
            let pairs = post::Entity::find()
                .find_also_related(user::Entity)
                .filter(post::Column::UserId.eq(user_id))
                .all(&fx.orm)
                .await
                .expect("select");
            black_box(pairs)
        });
    });
    group.bench_function("raw", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let user_id = i64::from(fx.user_id());
            let rows = raw_query(&fx.raw, POSTS_WITH_USER, [user_id]).await;
            let pairs = collect(rows, |row| {
                (
                    RawPost::from_row(row, 0),
                    RawUser::from_row_optional(row, 4),
                )
            })
            .await;
            black_box(pairs)
        });
    });
    group.finish();
}

/// One user with all their posts, grouped.
fn find_with_related(c: &mut Criterion, fx: &Fixture) {
    let mut group = c.benchmark_group("relations/find_with_related");
    group.bench_function("orm", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let user_id = fx.user_id();
            let grouped = user::Entity::find_by_id(user_id)
                .find_with_related(post::Entity)
                .all(&fx.orm)
                .await
                .expect("select");
            black_box(grouped)
        });
    });
    group.bench_function("raw", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let user_id = i64::from(fx.user_id());
            let rows = raw_query(&fx.raw, USER_WITH_POSTS, [user_id]).await;
            let flat = collect(rows, |row| {
                (RawUser::from_row(row, 0), RawPost::from_row(row, 3))
            })
            .await;
            let mut grouped: Vec<(RawUser, Vec<RawPost>)> = Vec::new();
            for (user, post) in flat {
                match grouped.last_mut() {
                    Some((last, posts)) if last.id == user.id => posts.push(post),
                    _ => grouped.push((user, vec![post])),
                }
            }
            black_box(grouped)
        });
    });
    group.finish();
}

/// The tags of ten posts, one query, answered in input order.
fn load_many(c: &mut Criterion, fx: &Fixture, posts: &[post::Model]) {
    let mut group = c.benchmark_group("relations/load_many");
    group.bench_function("orm", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let batch = posts_of(posts, fx.user_id());
            let tags = batch.load_many(tag::Entity, &fx.orm).await.expect("load");
            black_box(tags)
        });
    });
    group.bench_function("raw", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let batch = posts_of(posts, fx.user_id());
            let keys: Vec<RawValue> = batch
                .iter()
                .map(|p| RawValue::Integer(i64::from(p.id)))
                .collect();
            let rows = raw_query(&fx.raw, TAGS_OF_POSTS, keys).await;
            let flat = collect(rows, |row| (integer(row, 0), RawTag::from_row(row, 1))).await;
            let mut per_post: Vec<Vec<RawTag>> = vec![Vec::new(); batch.len()];
            for (post_id, tag) in flat {
                if let Some(slot) = batch.iter().position(|p| i64::from(p.id) == post_id) {
                    per_post[slot].push(tag);
                }
            }
            black_box(per_post)
        });
    });
    group.finish();
}

/// The distinct tags of a user's posts, three hops in one query.
fn linked(c: &mut Criterion, fx: &Fixture, users: &[user::Model]) {
    let mut group = c.benchmark_group("relations/linked");
    group.bench_function("orm", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let user = &users[index(fx.user_id())];
            let tags = user
                .find_linked(UserToTag)
                .distinct()
                .all(&fx.orm)
                .await
                .expect("select");
            black_box(tags)
        });
    });
    group.bench_function("raw", |b| {
        b.to_async(&fx.runtime).iter(|| async {
            let user_id = i64::from(fx.user_id());
            let rows = raw_query(&fx.raw, TAGS_OF_USER, [user_id]).await;
            black_box(collect(rows, |row| RawTag::from_row(row, 0)).await)
        });
    });
    group.finish();
}

/// Seeds once, reads the starting models once and runs every group of this file.
fn benches(c: &mut Criterion) {
    let fx = Fixture::load();
    let (posts, users) = fx.runtime.block_on(async {
        let posts = post::Entity::find()
            .order_by_asc(post::Column::Id)
            .all(&fx.orm)
            .await
            .expect("posts");
        let users = user::Entity::find()
            .order_by_asc(user::Column::Id)
            .all(&fx.orm)
            .await
            .expect("users");
        (posts, users)
    });
    assert_eq!(
        users.len(),
        usize::try_from(USERS).expect("fits"),
        "the seed must be intact"
    );
    find_also_related(c, &fx);
    find_with_related(c, &fx);
    load_many(c, &fx, &posts);
    linked(c, &fx, &users);
}

criterion_group!(relations, benches);
criterion_main!(relations);
