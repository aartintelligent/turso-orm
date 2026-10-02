//! End-to-end tests of the entity API against a real in-memory Turso database.
//!
//! The suite exercises the derive macros and the query builders together:
//! schema generation, single-row CRUD through active models, filtering,
//! ordering, pagination, counting and streaming, custom projections with
//! `FromQueryResult`, relations in every form (`find_related`,
//! `find_also_related`, joins and the batch loaders), composite keys and
//! transactions with savepoints. Each test opens its own database so that
//! they can run in parallel.
//!
//! ```text
//! cargo test -p turso-orm --test entity
//! ```

use futures_util::TryStreamExt;
use turso_orm::entity::LoaderTrait;
use turso_orm::prelude::*;
use turso_orm::query::UpdateResult;

/// The `user` entity: an auto-increment key, a unique column and one field of
/// every feature-gated type.
mod user {
    use turso_orm::prelude::*;

    /// A row of the `user` table.
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[turso(table_name = "user")]
    pub(crate) struct Model {
        #[turso(primary_key)]
        pub id: i32,
        #[turso(unique)]
        pub email: String,
        pub name: Option<String>,
        pub active: bool,
        pub score: f64,
        pub created_at: DateTime,
        pub token: Uuid,
        pub meta: Option<Json>,
    }

    /// The relations of `user`; the columns are inferred from the reverse
    /// `belongs_to` on `post`.
    #[derive(Copy, Clone, Debug, DeriveRelation)]
    pub(crate) enum Relation {
        #[turso(has_many = "super::post::Entity")]
        Post,
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// The `post` entity: owns the foreign key to `user` with `ON DELETE CASCADE`
/// and an indexed column.
mod post {
    use turso_orm::prelude::*;

    /// A row of the `post` table.
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[turso(table_name = "post")]
    pub(crate) struct Model {
        #[turso(primary_key)]
        pub id: i64,
        #[turso(indexed)]
        pub user_id: i32,
        pub title: String,
        pub views: i32,
    }

    /// The relations of `post`.
    #[derive(Copy, Clone, Debug, DeriveRelation)]
    pub(crate) enum Relation {
        #[turso(
            belongs_to = "super::user::Entity",
            from = "Column::UserId",
            to = "super::user::Column::Id",
            on_delete = "Cascade"
        )]
        User,
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// The `tag` entity: a composite primary key with a text component, so no
/// auto increment.
mod tag {
    use turso_orm::prelude::*;

    /// A row of the `tag` table.
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[turso(table_name = "tag")]
    pub(crate) struct Model {
        #[turso(primary_key)]
        pub post_id: i64,
        #[turso(primary_key)]
        pub label: String,
    }

    /// The relations of `tag`; there are none.
    #[derive(Copy, Clone, Debug, DeriveRelation)]
    pub(crate) enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// Opens a fresh in-memory database with the three tables and their indexes.
async fn setup() -> Database {
    let db = Database::connect(ConnectOptions::in_memory().max_connections(4))
        .await
        .expect("open");
    let schema = Schema::new();
    for stmt in [
        schema.create_table_from_entity(user::Entity).to_statement(),
        schema.create_table_from_entity(post::Entity).to_statement(),
        schema.create_table_from_entity(tag::Entity).to_statement(),
    ] {
        db.execute(stmt).await.expect("create table");
    }
    for idx in schema.create_index_from_entity(post::Entity) {
        db.execute(idx.to_statement()).await.expect("create index");
    }
    db
}

/// Builds a user active model with every non-key attribute `Set`, except `name`.
fn new_user(email: &str) -> user::ActiveModel {
    user::ActiveModel {
        email: Set(email.to_owned()),
        active: Set(true),
        score: Set(1.5),
        created_at: Set(chrono::NaiveDate::from_ymd_opt(2024, 1, 2)
            .expect("date")
            .and_hms_opt(3, 4, 5)
            .expect("time")),
        token: Set(Uuid::new_v4()),
        meta: Set(Some(serde_json::json!({"k": 1}))),
        ..Default::default()
    }
}

/// Schema generation renders inline and composite primary keys, foreign keys and `STRICT`.
#[test]
fn schema_ddl() {
    let sql = Schema::new()
        .create_table_from_entity(post::Entity)
        .to_string_inlined();
    assert_eq!(
        sql,
        "CREATE TABLE \"post\" (\"id\" INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, \"user_id\" INTEGER NOT NULL, \
         \"title\" TEXT NOT NULL, \"views\" INTEGER NOT NULL, \
         FOREIGN KEY (\"user_id\") REFERENCES \"user\" (\"id\") ON DELETE CASCADE)"
    );
    let sql = Schema::new()
        .strict(true)
        .create_table_from_entity(tag::Entity)
        .to_string_inlined();
    assert_eq!(
        sql,
        "CREATE TABLE \"tag\" (\"post_id\" INTEGER NOT NULL, \"label\" TEXT NOT NULL, PRIMARY KEY (\"post_id\", \"label\")) STRICT"
    );
}

/// Insert, find by id, update, save and delete round-trip a row through every column type.
#[tokio::test]
async fn crud() {
    let db = setup().await;
    let am = new_user("a@x.io");
    let token = am.token.clone().into_value().expect("set");
    let inserted = am.insert(&db).await.expect("insert");
    assert_eq!(inserted.id, 1);
    assert_eq!(inserted.email, "a@x.io");
    assert_eq!(inserted.token, token);
    assert_eq!(inserted.meta, Some(serde_json::json!({"k": 1})));
    assert_eq!(inserted.created_at.to_string(), "2024-01-02 03:04:05");

    let found = user::Entity::find_by_id(1)
        .one(&db)
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(found, inserted);

    // A model converted into an active model is all `Unchanged`, so it is
    // an update candidate with nothing to write yet.
    let mut am: user::ActiveModel = found.clone().into();
    assert!(am.is_update());
    assert!(!am.is_changed());
    am.name = Set(Some("Alice".into()));
    am.active = Set(false);
    let updated = am.update(&db).await.expect("update");
    assert_eq!(updated.name.as_deref(), Some("Alice"));
    assert!(!updated.active);

    // `save` with an `Unchanged` key updates and hands back the full row.
    let saved = user::ActiveModel {
        id: Unchanged(1),
        score: Set(9.0),
        ..Default::default()
    }
    .save(&db)
    .await
    .expect("save");
    assert_eq!(saved.score, Unchanged(9.0));
    assert_eq!(saved.name, Unchanged(Some("Alice".into())));

    let res = user::Entity::delete_by_id(1)
        .exec(&db)
        .await
        .expect("delete");
    assert_eq!(res.rows_affected, 1);
    assert!(
        user::Entity::find_by_id(1)
            .one(&db)
            .await
            .expect("find")
            .is_none()
    );
}

/// Filters, ordering, counting, pagination, streaming, `IN` lists and bulk updates and deletes behave as SQL does.
#[tokio::test]
async fn queries_filters_order_pagination_count_stream() {
    let db = setup().await;
    for i in 0..10 {
        let mut u = new_user(&format!("u{i}@x.io"));
        u.score = Set(f64::from(i));
        u.active = Set(i % 2 == 0);
        u.insert(&db).await.expect("insert");
    }
    let actives = user::Entity::find()
        .filter(user::Column::Active.eq(true))
        .filter(user::Column::Score.gte(4))
        .order_by_desc(user::Column::Score)
        .all(&db)
        .await
        .expect("all");
    assert_eq!(
        actives
            .iter()
            .map(|u| u.score.to_string())
            .collect::<Vec<_>>(),
        ["8", "6", "4"]
    );

    let cond = Condition::any()
        .add(user::Column::Email.starts_with("u1"))
        .add(user::Column::Email.ends_with("9@x.io"));
    assert_eq!(
        user::Entity::find()
            .filter(cond)
            .count(&db)
            .await
            .expect("count"),
        2
    );

    let paginator = user::Entity::find()
        .order_by_asc(user::Column::Id)
        .paginate(&db, 4);
    assert_eq!(paginator.num_items().await.expect("items"), 10);
    assert_eq!(paginator.num_pages().await.expect("pages"), 3);
    let page = paginator.fetch_page(2).await.expect("page");
    assert_eq!(page.iter().map(|u| u.id).collect::<Vec<_>>(), [9, 10]);

    let streamed: Vec<i32> = user::Entity::find()
        .order_by_asc(user::Column::Id)
        .limit(3)
        .stream(&db)
        .await
        .expect("stream")
        .map_ok(|u| u.id)
        .try_collect()
        .await
        .expect("collect");
    assert_eq!(streamed, [1, 2, 3]);

    let in_list = user::Entity::find()
        .filter(user::Column::Id.is_in([2, 3, 99]))
        .all(&db)
        .await
        .expect("in");
    assert_eq!(in_list.len(), 2);

    // Scores 0, 1 and 2 are below 3; of those, 0 and 2 were active.
    let UpdateResult { rows_affected } = user::Entity::update_many()
        .col(user::Column::Active, false)
        .filter(user::Column::Score.lt(3))
        .exec(&db)
        .await
        .expect("update many");
    assert_eq!(rows_affected, 3);
    let deleted = user::Entity::delete_many()
        .filter(user::Column::Active.eq(false))
        .exec(&db)
        .await
        .expect("delete many");
    assert_eq!(deleted.rows_affected, 7);
}

/// A custom projection decoded by name, with one field renamed through `column_name`.
#[derive(Debug, PartialEq, turso_orm::FromQueryResult)]
struct Stats {
    total: i64,
    #[turso(column_name = "max_score")]
    best: f64,
}

/// A multi-row insert returns every stored row, a bare select list decodes into a custom struct, and a unique violation is classified.
#[tokio::test]
async fn custom_select_into_model_and_insert_many() {
    let db = setup().await;
    let models: Vec<user::ActiveModel> = (0..3)
        .map(|i| {
            let mut u = new_user(&format!("m{i}@x.io"));
            u.score = Set(f64::from(i * 10));
            u
        })
        .collect();
    let inserted = user::Entity::insert_many(models)
        .exec_with_returning(&db)
        .await
        .expect("insert many");
    assert_eq!(inserted.len(), 3);

    let stats = user::Entity::find()
        .select_only()
        .expr_as(Func::count_star(), "total")
        .expr_as(user::Column::Score.max(), "max_score")
        .into_model::<Stats>()
        .one(&db)
        .await
        .expect("stats")
        .expect("row");
    assert_eq!(
        stats,
        Stats {
            total: 3,
            best: 20.0
        }
    );

    let err = new_user("m0@x.io")
        .insert(&db)
        .await
        .expect_err("duplicate email");
    assert!(err.is_unique_violation());
}

/// Relations resolve in both directions through `find_related`, `find_also_related`, joins and the loaders, and the generated foreign key cascades.
#[tokio::test]
async fn relations_and_loaders() {
    let db = setup().await;
    let alice = new_user("alice@x.io").insert(&db).await.expect("alice");
    let bob = new_user("bob@x.io").insert(&db).await.expect("bob");
    for (owner, title) in [(alice.id, "a1"), (alice.id, "a2"), (bob.id, "b1")] {
        post::ActiveModel {
            user_id: Set(owner),
            title: Set(title.into()),
            views: Set(0),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("post");
    }

    // Forward direction: a user's posts.
    let alice_posts = alice
        .find_related(post::Entity)
        .order_by_asc(post::Column::Id)
        .all(&db)
        .await
        .expect("related");
    assert_eq!(
        alice_posts
            .iter()
            .map(|p| p.title.as_str())
            .collect::<Vec<_>>(),
        ["a1", "a2"]
    );

    // Reverse direction: a post's owner.
    let first = post::Entity::find_by_id(1)
        .one(&db)
        .await
        .expect("p")
        .expect("exists");
    let owner = first
        .find_related(user::Entity)
        .one(&db)
        .await
        .expect("owner")
        .expect("exists");
    assert_eq!(owner.id, alice.id);

    // Both sides in one query through the `A_` / `B_` aliased select list.
    let pairs = post::Entity::find()
        .find_also_related(user::Entity)
        .order_by(post::Column::Id, Order::Asc)
        .all(&db)
        .await
        .expect("also related");
    assert_eq!(pairs.len(), 3);
    assert_eq!(
        pairs[2].1.as_ref().map(|u| u.email.as_str()),
        Some("bob@x.io")
    );

    // Batch loaders keep the result aligned with the input order.
    let users = user::Entity::find()
        .order_by_asc(user::Column::Id)
        .all(&db)
        .await
        .expect("users");
    let posts = users.load_many(post::Entity, &db).await.expect("load many");
    assert_eq!(posts.iter().map(Vec::len).collect::<Vec<_>>(), [2, 1]);

    let all_posts = post::Entity::find()
        .order_by_asc(post::Column::Id)
        .all(&db)
        .await
        .expect("posts");
    let owners = all_posts
        .load_one(user::Entity, &db)
        .await
        .expect("load one");
    assert_eq!(
        owners
            .iter()
            .map(|o| o.as_ref().map(|u| u.id))
            .collect::<Vec<_>>(),
        [Some(1), Some(1), Some(2)]
    );

    // A join lets a filter on the related table narrow the main one; the
    // qualified column references keep `user.id` and `post.id` apart.
    let joined = post::Entity::find()
        .inner_join(user::Entity)
        .filter(user::Column::Email.eq("bob@x.io"))
        .all(&db)
        .await
        .expect("join");
    assert_eq!(joined.len(), 1);

    // Deleting a user cascades to its posts through the generated foreign key.
    user::Entity::delete_by_id(alice.id)
        .exec(&db)
        .await
        .expect("delete alice");
    assert_eq!(post::Entity::find().count(&db).await.expect("count"), 1);
}

/// Composite keys round-trip as tuples, a rolled-back savepoint leaves no row, and the closure form of a transaction commits its result.
#[tokio::test]
async fn composite_keys_and_transactions() {
    let db = setup().await;
    let txn = db.begin().await.expect("begin");
    tag::ActiveModel {
        post_id: Set(7),
        label: Set("rust".into()),
    }
    .insert(&txn)
    .await
    .expect("insert tag");
    let found = tag::Entity::find_by_id((7, "rust".to_owned()))
        .one(&txn)
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(found.label, "rust");
    // A nested transaction is a savepoint; rolling it back drops only its
    // own insert.
    {
        let sp = txn.begin().await.expect("savepoint");
        tag::ActiveModel {
            post_id: Set(8),
            label: Set("ghost".into()),
        }
        .insert(&sp)
        .await
        .expect("insert");
        sp.rollback().await.expect("rollback");
    }
    assert_eq!(tag::Entity::find().count(&txn).await.expect("count"), 1);
    txn.commit().await.expect("commit");

    let n = db
        .transaction(|txn| {
            Box::pin(async move {
                tag::Entity::delete_by_id((7, "rust".to_owned()))
                    .exec(txn)
                    .await?;
                tag::Entity::find().count(txn).await
            })
        })
        .await
        .expect("closure");
    assert_eq!(n, 0);
}
