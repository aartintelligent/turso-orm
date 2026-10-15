//! Shared fixtures of the benchmarks: a blog schema, a deterministic data set and the two
//! databases every group compares, one behind the ORM and one behind the raw `turso` client.
//!
//! The ORM and the raw client each get their own in-memory database, created from the same
//! DDL and filled with the same rows: an in-memory database belongs to the client object that
//! opened it and cannot be shared with another one. The data set seeds in well under a second
//! and is large enough for a scan or a join to dominate the per-statement overhead: [`USERS`]
//! users with [`POSTS_PER_USER`] posts each, and [`TAGS_PER_POST`] tags on every post out of
//! [`TAGS`].
//!
//! The raw side goes through [`raw_query`] and [`raw_execute`], which prepare with the client's
//! statement cache exactly as the driver does, and decodes rows by hand into the `Raw*` structs,
//! the way an application would with the client alone, so that both sides of a group do the same
//! work on the engine and end with the same Rust values in hand.

use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};

use tokio::runtime::Runtime;
use turso::Value as RawValue;
use turso_orm::prelude::*;

/// The `user` entity: the owner of posts.
// The entity derives generate the `Column` and `PrimaryKey` variants and the `ActiveModel`
// fields without rustdoc, and `missing_docs` cannot be allowed on generated items alone.
#[allow(
    missing_docs,
    reason = "the entity derives generate undocumented variants and fields"
)]
pub mod user {
    use turso_orm::prelude::*;

    /// A row of the `user` table.
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[turso(table_name = "user")]
    pub struct Model {
        /// The primary key.
        #[turso(primary_key)]
        pub id: i32,
        /// The unique email.
        #[turso(unique)]
        pub email: String,
        /// The display name.
        pub name: String,
    }

    /// The posts a user wrote.
    #[derive(Copy, Clone, Debug, DeriveRelation)]
    pub enum Relation {
        /// The posts whose `user_id` is this user's id.
        #[turso(has_many = "super::post::Entity")]
        Posts,
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// The `post` entity: written by one user, tagged through the junction.
// The entity derives generate the `Column` and `PrimaryKey` variants and the `ActiveModel`
// fields without rustdoc, and `missing_docs` cannot be allowed on generated items alone.
#[allow(
    missing_docs,
    reason = "the entity derives generate undocumented variants and fields"
)]
pub mod post {
    use turso_orm::prelude::*;

    /// A row of the `post` table.
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[turso(table_name = "post")]
    pub struct Model {
        /// The primary key.
        #[turso(primary_key)]
        pub id: i32,
        /// The author's id, indexed.
        #[turso(indexed)]
        pub user_id: i32,
        /// The title.
        pub title: String,
        /// The body.
        pub body: String,
    }

    /// The relations of `post`: its author, the junction rows and the tags through them.
    #[derive(Copy, Clone, Debug, DeriveRelation)]
    pub enum Relation {
        /// The author.
        #[turso(
            belongs_to = "super::user::Entity",
            from = "Column::UserId",
            to = "super::user::Column::Id",
            on_delete = "Cascade"
        )]
        User,
        /// The junction rows of this post.
        #[turso(has_many = "super::post_tag::Entity")]
        PostTag,
        /// The tags of this post, through the junction.
        #[turso(has_many = "super::tag::Entity", via = "super::post_tag::Entity")]
        Tags,
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// The `tag` entity: a label shared by many posts.
// The entity derives generate the `Column` and `PrimaryKey` variants and the `ActiveModel`
// fields without rustdoc, and `missing_docs` cannot be allowed on generated items alone.
#[allow(
    missing_docs,
    reason = "the entity derives generate undocumented variants and fields"
)]
pub mod tag {
    use turso_orm::prelude::*;

    /// A row of the `tag` table.
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[turso(table_name = "tag")]
    pub struct Model {
        /// The primary key.
        #[turso(primary_key)]
        pub id: i32,
        /// The unique label.
        #[turso(unique)]
        pub label: String,
    }

    /// The relations of `tag`: the junction rows, and the posts through them.
    #[derive(Copy, Clone, Debug, DeriveRelation)]
    pub enum Relation {
        /// The junction rows of this tag.
        #[turso(has_many = "super::post_tag::Entity")]
        PostTag,
        /// The posts carrying this tag, through the junction.
        #[turso(has_many = "super::post::Entity", via = "super::post_tag::Entity")]
        Posts,
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// The junction between `post` and `tag`.
// The entity derives generate the `Column` and `PrimaryKey` variants and the `ActiveModel`
// fields without rustdoc, and `missing_docs` cannot be allowed on generated items alone.
#[allow(
    missing_docs,
    reason = "the entity derives generate undocumented variants and fields"
)]
pub mod post_tag {
    use turso_orm::prelude::*;

    /// A row of the `post_tag` table.
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[turso(table_name = "post_tag")]
    pub struct Model {
        /// The post side of the key.
        #[turso(primary_key)]
        pub post_id: i32,
        /// The tag side of the key.
        #[turso(primary_key)]
        pub tag_id: i32,
    }

    /// The two sides of the junction.
    #[derive(Copy, Clone, Debug, DeriveRelation)]
    pub enum Relation {
        /// The post.
        #[turso(
            belongs_to = "super::post::Entity",
            from = "Column::PostId",
            to = "super::post::Column::Id",
            on_delete = "Cascade"
        )]
        Post,
        /// The tag.
        #[turso(
            belongs_to = "super::tag::Entity",
            from = "Column::TagId",
            to = "super::tag::Column::Id",
            on_delete = "Cascade"
        )]
        Tag,
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// The chain from a user to the tags of the posts they wrote.
#[derive(Debug)]
pub struct UserToTag;

impl Linked for UserToTag {
    type FromEntity = user::Entity;
    type ToEntity = tag::Entity;

    fn link(&self) -> Vec<RelationDef> {
        vec![
            user::Relation::Posts.def(),
            post::Relation::PostTag.def(),
            post_tag::Relation::Tag.def(),
        ]
    }
}

/// Number of seeded users; their ids run from 1 to this value.
pub const USERS: u32 = 1_000;
/// Posts of each user, in id order: user `u` owns posts `(u - 1) * POSTS_PER_USER + 1` onwards.
pub const POSTS_PER_USER: u32 = 10;
/// Number of seeded posts; their ids run from 1 to this value.
pub const POSTS: u32 = USERS * POSTS_PER_USER;
/// Number of seeded tags; their ids run from 1 to this value.
pub const TAGS: u32 = 50;
/// Tags on every post, spread over the tag table so that each tag is used about as often.
pub const TAGS_PER_POST: u32 = 3;
/// Rows per multi-row `INSERT` while seeding; a statement this size stays far below the
/// engine's limits and keeps the number of round trips small.
const SEED_CHUNK: usize = 500;
/// The body every seeded post carries: long enough for decoding a text column to cost
/// something, short enough for the data set to stay small.
const BODY: &str = "An async ORM dedicated to the Turso database, with a typed entity API. \
    This body is the same on every seeded post so that the data set is deterministic.";

/// The two seeded databases and the runtime the benchmarks drive them on.
///
/// Ids for the point operations come from [`Fixture::post_id`] and [`Fixture::user_id`], which
/// walk the seeded range so that consecutive iterations never hit the same row.
pub struct Fixture {
    /// The current-thread runtime every async call runs on.
    pub runtime: Runtime,
    /// The ORM side: a pool over its own in-memory database.
    pub orm: Database,
    /// The raw side: one connection to its own in-memory database.
    pub raw: turso::Connection,
    /// The raw database object, kept alive for as long as its connection is used.
    raw_db: turso::Database,
    /// Position of the next id handed out by [`Fixture::post_id`] and [`Fixture::user_id`].
    cursor: AtomicU32,
}

impl fmt::Debug for Fixture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fixture")
            .field("orm", &self.orm)
            .field("raw", &self.raw)
            .field("cursor", &self.cursor)
            .finish_non_exhaustive()
    }
}

impl Fixture {
    /// Opens both databases, creates the schema in each and seeds the same rows into both.
    ///
    /// # Panics
    ///
    /// When the runtime cannot be built or either database refuses the schema or the seed,
    /// which means the benchmarks cannot run at all.
    pub fn load() -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        let statements: Vec<String> = schema_sql().into_iter().chain(seed_sql()).collect();
        let (orm, raw_db, raw) = runtime.block_on(async {
            let orm = Database::connect(ConnectOptions::in_memory())
                .await
                .expect("orm database");
            let raw_db = turso::Builder::new_local(":memory:")
                .build()
                .await
                .expect("raw database");
            let raw = raw_db.connect().expect("raw connection");
            apply_orm(&orm, &statements).await;
            apply_raw(&raw, &statements).await;
            (orm, raw_db, raw)
        });
        Self {
            runtime,
            orm,
            raw,
            raw_db,
            cursor: AtomicU32::new(0),
        }
    }

    /// The id of a seeded post, a different one on every call, cycling through the table.
    pub fn post_id(&self) -> i32 {
        fits(self.cursor.fetch_add(1, Ordering::Relaxed) % POSTS + 1)
    }

    /// The id of a seeded user, a different one on every call, cycling through the table.
    pub fn user_id(&self) -> i32 {
        fits(self.cursor.fetch_add(1, Ordering::Relaxed) % USERS + 1)
    }

    /// The raw database object; exposed so that the field counts as used.
    pub fn raw_database(&self) -> &turso::Database {
        &self.raw_db
    }
}

/// The DDL of the four tables and the index on `post.user_id`, rendered from the entities so
/// that both databases get exactly the schema the ORM expects.
fn schema_sql() -> Vec<String> {
    let schema = Schema::new();
    let mut statements = vec![
        schema
            .create_table_from_entity(user::Entity)
            .to_statement()
            .sql,
        schema
            .create_table_from_entity(post::Entity)
            .to_statement()
            .sql,
        schema
            .create_table_from_entity(tag::Entity)
            .to_statement()
            .sql,
        schema
            .create_table_from_entity(post_tag::Entity)
            .to_statement()
            .sql,
    ];
    statements.extend(
        schema
            .create_index_from_entity(post::Entity)
            .into_iter()
            .map(|index| index.to_statement().sql),
    );
    statements
}

/// The seed as multi-row `INSERT` statements with inlined literals, the same text for both
/// databases. Every value is a number or a plain ASCII word, so nothing needs escaping.
fn seed_sql() -> Vec<String> {
    let users = (1..=USERS).map(|u| format!("({u}, 'user{u}@example.com', 'User {u}')"));
    let posts = (1..=POSTS).map(|p| {
        let user = (p - 1) / POSTS_PER_USER + 1;
        format!("({p}, {user}, 'Post {p}', '{BODY}')")
    });
    let tags = (1..=TAGS).map(|t| format!("({t}, 'tag{t}')"));
    let post_tags = (1..=POSTS).flat_map(|p| (0..TAGS_PER_POST).map(move |k| (p, tag_of(p, k))));
    let post_tags = post_tags.map(|(p, t)| format!("({p}, {t})"));

    let mut statements = Vec::new();
    statements.extend(inserts("user", "id, email, name", users));
    statements.extend(inserts("post", "id, user_id, title, body", posts));
    statements.extend(inserts("tag", "id, label", tags));
    statements.extend(inserts("post_tag", "post_id, tag_id", post_tags));
    statements
}

/// The `k`-th tag of post `p`: three tags seven apart, so that the three are distinct and
/// every tag ends up on about the same number of posts.
fn tag_of(p: u32, k: u32) -> u32 {
    (p + k * 7) % TAGS + 1
}

/// Groups rendered value tuples into multi-row `INSERT` statements of [`SEED_CHUNK`] rows.
fn inserts(table: &str, columns: &str, rows: impl Iterator<Item = String>) -> Vec<String> {
    rows.collect::<Vec<_>>()
        .chunks(SEED_CHUNK)
        .map(|chunk| {
            format!(
                "INSERT INTO {table} ({columns}) VALUES {}",
                chunk.join(", ")
            )
        })
        .collect()
}

/// Runs the statements on the ORM side in one transaction.
async fn apply_orm(db: &Database, statements: &[String]) {
    let tx = db.begin().await.expect("begin");
    for sql in statements {
        tx.execute_unprepared(sql).await.expect("seed statement");
    }
    tx.commit().await.expect("commit");
}

/// Runs the statements on the raw side in one transaction.
async fn apply_raw(conn: &turso::Connection, statements: &[String]) {
    conn.execute("BEGIN", ()).await.expect("begin");
    for sql in statements {
        conn.execute(sql, ()).await.expect("seed statement");
    }
    conn.execute("COMMIT", ()).await.expect("commit");
}

/// Narrows a seeded id to the `i32` the entities use; every seeded id fits by construction.
fn fits(id: u32) -> i32 {
    i32::try_from(id).expect("seeded ids fit in i32")
}

/// A `post` row read through the raw client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawPost {
    /// The primary key.
    pub id: i64,
    /// The author's id.
    pub user_id: i64,
    /// The title.
    pub title: String,
    /// The body.
    pub body: String,
}

impl RawPost {
    /// Decodes the four columns of a post starting at `offset` in the row.
    pub fn from_row(row: &turso::Row, offset: usize) -> Self {
        Self {
            id: integer(row, offset),
            user_id: integer(row, offset + 1),
            title: text(row, offset + 2),
            body: text(row, offset + 3),
        }
    }
}

/// A `user` row read through the raw client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawUser {
    /// The primary key.
    pub id: i64,
    /// The unique email.
    pub email: String,
    /// The display name.
    pub name: String,
}

impl RawUser {
    /// Decodes the three columns of a user starting at `offset` in the row.
    pub fn from_row(row: &turso::Row, offset: usize) -> Self {
        Self {
            id: integer(row, offset),
            email: text(row, offset + 1),
            name: text(row, offset + 2),
        }
    }

    /// Decodes a user from the nullable side of a `LEFT JOIN`: `None` when its key is `NULL`.
    pub fn from_row_optional(row: &turso::Row, offset: usize) -> Option<Self> {
        nullable_integer(row, offset).map(|_| Self::from_row(row, offset))
    }
}

/// A `tag` row read through the raw client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawTag {
    /// The primary key.
    pub id: i64,
    /// The unique label.
    pub label: String,
}

impl RawTag {
    /// Decodes the two columns of a tag starting at `offset` in the row.
    pub fn from_row(row: &turso::Row, offset: usize) -> Self {
        Self {
            id: integer(row, offset),
            label: text(row, offset + 1),
        }
    }
}

/// Reads an `INTEGER` column of a raw row.
///
/// # Panics
///
/// When the column is missing or holds another storage class: the benchmark queries are
/// fixed, so that is a bug in the benchmark, not a runtime condition.
pub fn integer(row: &turso::Row, idx: usize) -> i64 {
    match row.get_value(idx).expect("column in range") {
        RawValue::Integer(value) => value,
        other => panic!("column {idx} is not an integer: {other:?}"),
    }
}

/// Reads a nullable `INTEGER` column of a raw row.
///
/// # Panics
///
/// When the column is missing or holds a storage class other than `INTEGER` or `NULL`.
pub fn nullable_integer(row: &turso::Row, idx: usize) -> Option<i64> {
    match row.get_value(idx).expect("column in range") {
        RawValue::Integer(value) => Some(value),
        RawValue::Null => None,
        other => panic!("column {idx} is not an integer or null: {other:?}"),
    }
}

/// Reads a `TEXT` column of a raw row.
///
/// # Panics
///
/// When the column is missing or holds another storage class.
pub fn text(row: &turso::Row, idx: usize) -> String {
    match row.get_value(idx).expect("column in range") {
        RawValue::Text(value) => value,
        other => panic!("column {idx} is not text: {other:?}"),
    }
}

/// Runs a raw query through the client's statement cache, as the driver does for every statement.
///
/// # Panics
///
/// When the statement fails to prepare or to start.
pub async fn raw_query(
    conn: &turso::Connection,
    sql: &str,
    params: impl turso::IntoParams,
) -> turso::Rows {
    let mut statement = conn.prepare_cached(sql).await.expect("prepare");
    statement.query(params).await.expect("query")
}

/// Runs a raw write through the client's statement cache and returns the number of rows changed.
///
/// # Panics
///
/// When the statement fails to prepare or to run.
pub async fn raw_execute(
    conn: &turso::Connection,
    sql: &str,
    params: impl turso::IntoParams,
) -> u64 {
    let mut statement = conn.prepare_cached(sql).await.expect("prepare");
    statement.execute(params).await.expect("execute")
}

/// Drains a raw result set, decoding every row with `decode`.
///
/// # Panics
///
/// When the engine reports an error while stepping the statement.
pub async fn collect<T>(mut rows: turso::Rows, decode: impl Fn(&turso::Row) -> T) -> Vec<T> {
    let mut out = Vec::new();
    while let Some(row) = rows.next().await.expect("next row") {
        out.push(decode(&row));
    }
    out
}

/// Reads the first row of a raw result set, if any, decoding it with `decode`.
///
/// # Panics
///
/// When the engine reports an error while stepping the statement.
pub async fn first<T>(mut rows: turso::Rows, decode: impl Fn(&turso::Row) -> T) -> Option<T> {
    rows.next().await.expect("next row").map(|row| decode(&row))
}
