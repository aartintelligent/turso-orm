//! End-to-end tests of the relational features against a real in-memory Turso database.
//!
//! Where `entity.rs` covers the basics, this suite exercises what a larger
//! schema needs: many-to-many relations through a junction table,
//! self-referencing relations, several relations to the same entity,
//! multi-hop chains, enum columns, partial models, request structs
//! converted into active models, JSON input, keyset pagination, tuple and
//! JSON projections, `RETURNING` deletes and wide composite keys. Each test
//! opens its own database so that they can run in parallel.
//!
//! ```text
//! cargo test -p turso-orm --test relational
//! ```

use turso_orm::entity::LoaderTrait;
use turso_orm::prelude::*;
use turso_orm::query::UpdateResult;

/// The `user` entity, the author or editor of posts.
mod user {
    use turso_orm::prelude::*;

    /// A row of the `user` table.
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[turso(table_name = "user")]
    pub(crate) struct Model {
        #[turso(primary_key)]
        pub id: i32,
        #[turso(unique)]
        pub name: String,
    }

    /// The relations of `user`: the posts it authored, inferred from the
    /// first relation `post` declares back to `user`.
    #[derive(Copy, Clone, Debug, DeriveRelation)]
    pub(crate) enum Relation {
        #[turso(has_many = "super::post::Entity")]
        Posts,
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// The `post` entity: two foreign keys to `user`, a self-reference, two
/// enum columns, a declared default and an ignored field.
mod post {
    use turso_orm::prelude::*;

    /// The publication state, stored as text.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, DeriveActiveEnum)]
    #[turso(rs_type = "String")]
    pub(crate) enum Status {
        Draft,
        #[turso(string_value = "live")]
        Published,
    }

    /// The priority, stored as an integer.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, DeriveActiveEnum)]
    #[turso(rs_type = "i32")]
    pub(crate) enum Priority {
        #[turso(num_value = 1)]
        Low,
        #[turso(num_value = 5)]
        High,
    }

    /// A row of the `post` table.
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[turso(table_name = "post")]
    pub(crate) struct Model {
        #[turso(primary_key)]
        pub id: i32,
        #[turso(indexed)]
        pub author_id: i32,
        pub editor_id: Option<i32>,
        pub parent_id: Option<i32>,
        pub title: String,
        #[turso(default_value = "draft")]
        pub status: Status,
        pub priority: Priority,
        #[turso(ignore)]
        pub cached_len: usize,
    }

    /// The relations of `post`. `Author` is the one `Related<user::Entity>`
    /// names; `Editor` is reached through its `def()`. `Parent` provides
    /// `Related<Entity>` and `Children` is its reverse.
    #[derive(Copy, Clone, Debug, DeriveRelation)]
    pub(crate) enum Relation {
        #[turso(
            belongs_to = "super::user::Entity",
            from = "Column::AuthorId",
            to = "super::user::Column::Id",
            on_delete = "Cascade"
        )]
        Author,
        #[turso(
            belongs_to = "super::user::Entity",
            from = "Column::EditorId",
            to = "super::user::Column::Id",
            on_delete = "SetNull"
        )]
        Editor,
        #[turso(
            belongs_to = "Entity",
            from = "Column::ParentId",
            to = "Column::Id",
            on_delete = "Cascade"
        )]
        Parent,
        #[turso(has_many = "Entity")]
        Children,
        #[turso(has_many = "super::post_category::Entity")]
        PostCategory,
        #[turso(
            has_many = "super::category::Entity",
            via = "super::post_category::Entity"
        )]
        Categories,
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// The `category` entity, the other side of the many-to-many relation.
mod category {
    use turso_orm::prelude::*;

    /// A row of the `category` table.
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[turso(table_name = "category")]
    pub(crate) struct Model {
        #[turso(primary_key)]
        pub id: i32,
        #[turso(unique)]
        pub name: String,
    }

    /// The relations of `category`.
    #[derive(Copy, Clone, Debug, DeriveRelation)]
    pub(crate) enum Relation {
        #[turso(has_many = "super::post_category::Entity")]
        PostCategory,
        #[turso(has_many = "super::post::Entity", via = "super::post_category::Entity")]
        Posts,
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// The junction between `post` and `category`.
mod post_category {
    use turso_orm::prelude::*;

    /// A row of the `post_category` table.
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[turso(table_name = "post_category")]
    pub(crate) struct Model {
        #[turso(primary_key)]
        pub post_id: i32,
        #[turso(primary_key)]
        pub category_id: i32,
    }

    /// The two `belongs_to` relations the many-to-many hops are built from.
    #[derive(Copy, Clone, Debug, DeriveRelation)]
    pub(crate) enum Relation {
        #[turso(
            belongs_to = "super::post::Entity",
            from = "Column::PostId",
            to = "super::post::Column::Id",
            on_delete = "Cascade"
        )]
        Post,
        #[turso(
            belongs_to = "super::category::Entity",
            from = "Column::CategoryId",
            to = "super::category::Column::Id",
            on_delete = "Cascade"
        )]
        Category,
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// An entity with a four-column composite key.
mod quad {
    use turso_orm::prelude::*;

    /// A row of the `quad` table.
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[turso(table_name = "quad")]
    pub(crate) struct Model {
        #[turso(primary_key)]
        pub a: i32,
        #[turso(primary_key)]
        pub b: i32,
        #[turso(primary_key)]
        pub c: String,
        #[turso(primary_key)]
        pub d: i32,
        pub note: Option<String>,
    }

    /// The relations of `quad`; there are none.
    #[derive(Copy, Clone, Debug, DeriveRelation)]
    pub(crate) enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// The chain from a user to the categories of the posts it authored.
struct UserToCategory;

impl Linked for UserToCategory {
    type FromEntity = user::Entity;
    type ToEntity = category::Entity;

    fn link(&self) -> Vec<RelationDef> {
        vec![
            user::Relation::Posts.def(),
            post::Relation::PostCategory.def(),
            post_category::Relation::Category.def(),
        ]
    }
}

/// A projection of `post` with a renamed column and a computed expression.
#[derive(Debug, PartialEq, DerivePartialModel)]
#[turso(entity = "post::Entity")]
struct PostTitle {
    id: i32,
    #[turso(from_col = "Title")]
    name: String,
    #[turso(from_expr = "Expr::col((\"post\", \"title\")).concat(\"!\")")]
    shout: String,
}

/// A request body for creating a post.
#[derive(Debug, DeriveIntoActiveModel)]
#[turso(active_model = "post::ActiveModel")]
struct NewPost {
    author_id: i32,
    title: String,
    editor_id: Option<i32>,
    // The outer `Option` is the "leave the column alone" signal of
    // `IntoActiveValue`; the inner one is the nullable column type.
    #[allow(
        clippy::option_option,
        reason = "the outer Option means NotSet, the inner one is the column type"
    )]
    parent_id: Option<Option<i32>>,
    priority: Option<post::Priority>,
    #[turso(ignore)]
    extra: bool,
}

/// Opens a fresh in-memory database with every table and index.
async fn setup() -> Database {
    let db = Database::connect(ConnectOptions::in_memory().max_connections(4))
        .await
        .expect("open");
    let schema = Schema::new();
    for stmt in [
        schema.create_table_from_entity(user::Entity).to_statement(),
        schema.create_table_from_entity(post::Entity).to_statement(),
        schema
            .create_table_from_entity(category::Entity)
            .to_statement(),
        schema
            .create_table_from_entity(post_category::Entity)
            .to_statement(),
        schema.create_table_from_entity(quad::Entity).to_statement(),
    ] {
        db.execute(stmt).await.expect("create table");
    }
    for idx in schema.create_index_from_entity(post::Entity) {
        db.execute(idx.to_statement()).await.expect("create index");
    }
    db
}

/// Inserts a user and returns it.
async fn user(db: &Database, name: &str) -> user::Model {
    user::ActiveModel {
        name: Set(name.to_owned()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("user")
}

/// Inserts a published post by `author` and returns it.
async fn post(db: &Database, author: i32, title: &str) -> post::Model {
    post::ActiveModel {
        author_id: Set(author),
        title: Set(title.to_owned()),
        status: Set(post::Status::Published),
        priority: Set(post::Priority::Low),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("post")
}

/// Inserts a category and returns it.
async fn category(db: &Database, name: &str) -> category::Model {
    category::ActiveModel {
        name: Set(name.to_owned()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("category")
}

/// Links a post and a category in the junction table.
async fn tag(db: &Database, post: i32, category: i32) {
    post_category::ActiveModel {
        post_id: Set(post),
        category_id: Set(category),
    }
    .insert(db)
    .await
    .expect("junction");
}

/// The DDL carries the enum column types, the declared default and every foreign key, but no foreign key for the junction-based relation, and an ignored field decodes to its default.
#[tokio::test]
async fn schema_enums_default_and_ignored_field() {
    let ddl = Schema::new()
        .create_table_from_entity(post::Entity)
        .to_string_inlined();
    assert!(
        ddl.contains("\"status\" TEXT NOT NULL DEFAULT 'draft'"),
        "{ddl}"
    );
    assert!(ddl.contains("\"priority\" INTEGER NOT NULL"), "{ddl}");
    assert_eq!(ddl.matches("FOREIGN KEY").count(), 3, "{ddl}");
    assert!(!ddl.contains("cached_len"), "{ddl}");

    let db = setup().await;
    let alice = user(&db, "alice").await;
    let found = post(&db, alice.id, "hello").await;
    assert_eq!(found.cached_len, 0);
    assert_eq!(found.status, post::Status::Published);
    assert_eq!(found.priority, post::Priority::Low);
}

/// `contains`, `starts_with` and `ends_with` match `%` and `_` literally, while `like` keeps them as wildcards.
#[tokio::test]
async fn like_fragments_match_wildcards_literally() {
    let db = setup().await;
    let alice = user(&db, "alice").await;
    for title in ["100%", "100 percent", "a_b", "axb", "back\\slash"] {
        post(&db, alice.id, title).await;
    }
    let titles = |rows: Vec<post::Model>| rows.into_iter().map(|p| p.title).collect::<Vec<_>>();

    let percent = post::Entity::find()
        .filter(post::Column::Title.contains("%"))
        .all(&db)
        .await
        .expect("contains");
    assert_eq!(titles(percent), ["100%"]);

    let underscore = post::Entity::find()
        .filter(post::Column::Title.starts_with("a_"))
        .all(&db)
        .await
        .expect("starts_with");
    assert_eq!(titles(underscore), ["a_b"]);

    let backslash = post::Entity::find()
        .filter(post::Column::Title.ends_with("\\slash"))
        .all(&db)
        .await
        .expect("ends_with");
    assert_eq!(titles(backslash), ["back\\slash"]);

    let wildcard = post::Entity::find()
        .filter(post::Column::Title.like("a_b"))
        .order_by_asc(post::Column::Id)
        .all(&db)
        .await
        .expect("like");
    assert_eq!(titles(wildcard), ["a_b", "axb"]);
}

/// A bulk insert fills a column one model leaves unset with the default the entity declares, and with `NULL` when it declares none.
#[tokio::test]
async fn insert_many_applies_declared_defaults() {
    let db = setup().await;
    let alice = user(&db, "alice").await;
    let rows = post::Entity::insert_many([
        post::ActiveModel {
            author_id: Set(alice.id),
            title: Set("explicit".into()),
            status: Set(post::Status::Published),
            priority: Set(post::Priority::High),
            editor_id: Set(Some(alice.id)),
            ..Default::default()
        },
        post::ActiveModel {
            author_id: Set(alice.id),
            title: Set("defaulted".into()),
            priority: Set(post::Priority::Low),
            ..Default::default()
        },
    ])
    .exec_with_returning(&db)
    .await
    .expect("insert many");
    assert_eq!(rows[0].status, post::Status::Published);
    assert_eq!(rows[0].editor_id, Some(alice.id));
    assert_eq!(rows[1].status, post::Status::Draft);
    assert_eq!(rows[1].editor_id, None);
}

/// A many-to-many relation is followed through its junction by `find_related` in both directions, by the joins, by `find_also_related`, by `find_with_related` and by the loaders.
#[tokio::test]
async fn many_to_many_through_junction() {
    let db = setup().await;
    let alice = user(&db, "alice").await;
    let p1 = post(&db, alice.id, "p1").await;
    let p2 = post(&db, alice.id, "p2").await;
    let p3 = post(&db, alice.id, "p3").await;
    let rust = category(&db, "rust").await;
    let db_cat = category(&db, "databases").await;
    tag(&db, p1.id, rust.id).await;
    tag(&db, p1.id, db_cat.id).await;
    tag(&db, p2.id, rust.id).await;

    let names = |rows: Vec<category::Model>| rows.into_iter().map(|c| c.name).collect::<Vec<_>>();
    let p1_categories = p1
        .find_related(category::Entity)
        .order_by_asc(category::Column::Id)
        .all(&db)
        .await
        .expect("find_related via");
    assert_eq!(names(p1_categories), ["rust", "databases"]);

    let rust_posts = rust
        .find_related(post::Entity)
        .order_by_asc(post::Column::Id)
        .all(&db)
        .await
        .expect("reverse via");
    assert_eq!(
        rust_posts.iter().map(|p| p.id).collect::<Vec<_>>(),
        [p1.id, p2.id]
    );

    let joined = post::Entity::find()
        .inner_join(category::Entity)
        .filter(category::Column::Name.eq("databases"))
        .all(&db)
        .await
        .expect("inner_join via");
    assert_eq!(joined.len(), 1);
    assert_eq!(joined[0].id, p1.id);

    let pairs = post::Entity::find()
        .find_also_related(category::Entity)
        .order_by(post::Column::Id, Order::Asc)
        .order_by_related(category::Column::Id, Order::Asc)
        .all(&db)
        .await
        .expect("find_also_related via");
    assert_eq!(pairs.len(), 4);
    assert_eq!(pairs[3].0.id, p3.id);
    assert!(pairs[3].1.is_none());

    let grouped = post::Entity::find()
        .find_with_related(category::Entity)
        .order_by(post::Column::Id, Order::Asc)
        .order_by_related(category::Column::Id, Order::Asc)
        .all(&db)
        .await
        .expect("find_with_related");
    assert_eq!(
        grouped
            .iter()
            .map(|(p, cs)| (p.id, cs.iter().map(|c| c.name.as_str()).collect::<Vec<_>>()))
            .collect::<Vec<_>>(),
        [
            (p1.id, vec!["rust", "databases"]),
            (p2.id, vec!["rust"]),
            (p3.id, vec![])
        ]
    );

    let posts = post::Entity::find()
        .order_by_asc(post::Column::Id)
        .all(&db)
        .await
        .expect("posts");
    let loaded = posts
        .load_many(category::Entity, &db)
        .await
        .expect("load_many via");
    assert_eq!(loaded.iter().map(Vec::len).collect::<Vec<_>>(), [2, 1, 0]);
    let explicit = posts
        .load_many_to_many(category::Entity, &db)
        .await
        .expect("load_many_to_many");
    assert_eq!(explicit, loaded);

    let direct = posts.load_many_to_many(user::Entity, &db).await;
    assert!(
        matches!(direct, Err(DbErr::Custom(ref m)) if m.contains("junction")),
        "{direct:?}"
    );
}

/// A self-referencing relation and two relations to the same entity are reachable through `Related` for the first of each and through `def()` for the others, with aliased self-joins.
#[tokio::test]
async fn self_reference_and_several_relations_to_one_entity() {
    let db = setup().await;
    let alice = user(&db, "alice").await;
    let bob = user(&db, "bob").await;
    let root = post(&db, alice.id, "root").await;
    let child = post::ActiveModel {
        author_id: Set(alice.id),
        editor_id: Set(Some(bob.id)),
        parent_id: Set(Some(root.id)),
        title: Set("child".into()),
        status: Set(post::Status::Draft),
        priority: Set(post::Priority::High),
        ..Default::default()
    }
    .insert(&db)
    .await
    .expect("child");

    // `Related<post::Entity>` is the `Parent` relation.
    let parent = child
        .find_related(post::Entity)
        .one(&db)
        .await
        .expect("parent")
        .expect("exists");
    assert_eq!(parent.id, root.id);

    // The reverse side goes through its definition.
    let children = post::Entity::find()
        .related_to(&post::Relation::Children.def(), &root)
        .all(&db)
        .await
        .expect("children");
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].id, child.id);

    // `Related<user::Entity>` is `Author`; `Editor` is used by its definition.
    let author = child
        .find_related(user::Entity)
        .one(&db)
        .await
        .expect("author")
        .expect("exists");
    assert_eq!(author.id, alice.id);
    let edited = post::Entity::find()
        .related_to(&post::Relation::Editor.def().rev(), &bob)
        .all(&db)
        .await
        .expect("edited by bob");
    assert_eq!(edited.len(), 1);
    assert_eq!(edited[0].id, child.id);

    // A chosen alias lets a filter name the joined side.
    let by_editor = post::Entity::find()
        .join_as(JoinType::Inner, &post::Relation::Editor.def(), "editor")
        .filter(Expr::col(("editor", "name")).eq("bob"))
        .all(&db)
        .await
        .expect("join_as");
    assert_eq!(by_editor.len(), 1);

    // A self-join aliases the second occurrence of the table `post_1`.
    let sql = post::Entity::find().inner_join(post::Entity).build().sql;
    assert!(
        sql.contains(
            "INNER JOIN \"post\" AS \"post_1\" ON \"post\".\"parent_id\" = \"post_1\".\"id\""
        ),
        "{sql}"
    );
    let of_root = post::Entity::find()
        .inner_join(post::Entity)
        .filter(Expr::col(("post_1", "title")).eq("root"))
        .all(&db)
        .await
        .expect("self join");
    assert_eq!(of_root.len(), 1);
    assert_eq!(of_root[0].id, child.id);

    let pairs = post::Entity::find()
        .find_also_related(post::Entity)
        .order_by(post::Column::Id, Order::Asc)
        .all(&db)
        .await
        .expect("self find_also_related");
    assert_eq!(pairs.len(), 2);
    assert!(pairs[0].1.is_none());
    assert_eq!(pairs[1].1.as_ref().map(|p| p.id), Some(root.id));

    // Users see their posts through the inferred `has_many`.
    let alices = alice
        .find_related(post::Entity)
        .count(&db)
        .await
        .expect("count");
    assert_eq!(alices, 2);
}

/// A `Linked` chain reaches the entity at its end from a model, alongside each model and grouped per model.
#[tokio::test]
async fn linked_chains() {
    let db = setup().await;
    let alice = user(&db, "alice").await;
    let bob = user(&db, "bob").await;
    let p1 = post(&db, alice.id, "p1").await;
    let p2 = post(&db, alice.id, "p2").await;
    let p3 = post(&db, bob.id, "p3").await;
    let rust = category(&db, "rust").await;
    let db_cat = category(&db, "databases").await;
    tag(&db, p1.id, rust.id).await;
    tag(&db, p2.id, rust.id).await;
    tag(&db, p3.id, db_cat.id).await;

    let categories = alice
        .find_linked(UserToCategory)
        .distinct()
        .all(&db)
        .await
        .expect("find_linked");
    assert_eq!(categories.len(), 1);
    assert_eq!(categories[0].name, "rust");

    let pairs = user::Entity::find()
        .find_also_linked(&UserToCategory)
        .order_by(user::Column::Id, Order::Asc)
        .all(&db)
        .await
        .expect("find_also_linked");
    assert_eq!(pairs.len(), 3);
    assert_eq!(
        pairs[2].1.as_ref().map(|c| c.name.as_str()),
        Some("databases")
    );

    let grouped = user::Entity::find()
        .find_with_linked(&UserToCategory)
        .order_by(user::Column::Id, Order::Asc)
        .all(&db)
        .await
        .expect("find_with_linked");
    assert_eq!(grouped.len(), 2);
    assert_eq!(grouped[0].1.len(), 2);
    assert_eq!(grouped[1].1.len(), 1);

    let carol = user(&db, "carol").await;
    let none = carol
        .find_linked(UserToCategory)
        .all(&db)
        .await
        .expect("empty chain");
    assert!(none.is_empty());
}

/// Enum columns bind in conditions and reject unknown stored values, partial models select and decode their own columns, request structs convert into active models, and active models convert back into models.
#[tokio::test]
async fn enums_partial_models_and_conversions() {
    let db = setup().await;
    let alice = user(&db, "alice").await;
    let live = post(&db, alice.id, "live").await;
    post::ActiveModel {
        author_id: Set(alice.id),
        title: Set("draft".into()),
        status: Set(post::Status::Draft),
        priority: Set(post::Priority::High),
        ..Default::default()
    }
    .insert(&db)
    .await
    .expect("draft");

    assert_eq!(
        post::Status::values(),
        [post::Status::Draft, post::Status::Published]
    );
    let published = post::Entity::find()
        .filter(post::Column::Status.eq(post::Status::Published))
        .all(&db)
        .await
        .expect("filter by enum");
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].id, live.id);
    let high = post::Entity::find()
        .filter(post::Column::Priority.is_in([post::Priority::High]))
        .count(&db)
        .await
        .expect("count by enum");
    assert_eq!(high, 1);

    // A value no variant maps to is a decoding error, not a silent default.
    db.execute(Statement::from_string(
        "UPDATE post SET status = 'bogus' WHERE title = 'draft'",
    ))
    .await
    .expect("corrupt");
    let bad = post::Entity::find()
        .filter(post::Column::Title.eq("draft"))
        .one(&db)
        .await;
    assert!(matches!(bad, Err(DbErr::Driver(_))), "{bad:?}");

    let titles = post::Entity::find()
        .filter(post::Column::Id.eq(live.id))
        .into_partial_model::<PostTitle>()
        .all(&db)
        .await
        .expect("partial");
    assert_eq!(
        titles,
        [PostTitle {
            id: live.id,
            name: "live".into(),
            shout: "live!".into()
        }]
    );

    let request = NewPost {
        author_id: alice.id,
        title: "from request".into(),
        editor_id: None,
        parent_id: None,
        priority: Some(post::Priority::High),
        extra: true,
    };
    assert!(request.extra);
    let am = request.into_active_model();
    assert_eq!(am.editor_id, ActiveValue::Set(None));
    assert!(am.parent_id.is_not_set());
    assert_eq!(am.priority, ActiveValue::Set(post::Priority::High));
    assert!(am.status.is_not_set());
    let created = am.insert(&db).await.expect("insert from request");
    assert_eq!(created.status, post::Status::Draft);

    let full: post::ActiveModel = created.clone().into();
    let back = full.try_into_model().expect("complete");
    assert_eq!(back, created);
    let partial = post::ActiveModel {
        title: Set("x".into()),
        ..Default::default()
    };
    assert!(matches!(
        partial.try_into_model(),
        Err(DbErr::AttrNotSet(ref c)) if c == "id"
    ));

    let removed = created.delete(&db).await.expect("model delete");
    assert_eq!(removed.rows_affected, 1);
}

/// JSON input fills an active model by column name and JSON output mirrors a row, tuples decode by position, `exists` probes without reading rows, and deletes can return the removed rows.
#[tokio::test]
async fn json_tuples_exists_and_returning() {
    let db = setup().await;
    let alice = user(&db, "alice").await;

    let am = post::ActiveModel::from_json(serde_json::json!({
        "author_id": alice.id,
        "title": "json",
        "status": "live",
        "priority": 5,
        "unknown": "ignored"
    }))
    .expect("from_json");
    assert_eq!(am.priority, ActiveValue::Set(post::Priority::High));
    let created = am.insert(&db).await.expect("insert");
    assert_eq!(created.status, post::Status::Published);
    let not_object = post::ActiveModel::from_json(serde_json::json!([1]));
    assert!(matches!(not_object, Err(DbErr::Json(_))));
    let wrong_type = post::ActiveModel::from_json(serde_json::json!({"author_id": "abc"}));
    assert!(matches!(wrong_type, Err(DbErr::Type(_))));

    let json = post::Entity::find()
        .into_json()
        .one(&db)
        .await
        .expect("into_json")
        .expect("row");
    assert_eq!(json["title"], "json");
    assert_eq!(json["priority"], 5);
    assert_eq!(json["editor_id"], serde_json::Value::Null);

    let tuples = post::Entity::find()
        .select_only()
        .column(post::Column::Id)
        .column_as(post::Column::Title, "t")
        .expr(post::Column::Priority.avg())
        .into_tuple::<(i32, String, f64)>()
        .all(&db)
        .await
        .expect("into_tuple");
    assert_eq!(tuples, [(created.id, "json".to_owned(), 5.0)]);

    assert!(
        post::Entity::find()
            .filter(post::Column::Title.eq("json"))
            .exists(&db)
            .await
            .expect("exists")
    );
    assert!(
        !post::Entity::find()
            .filter(post::Column::Title.eq("nope"))
            .exists(&db)
            .await
            .expect("exists")
    );

    let second = post(&db, alice.id, "second").await;
    let bumped: UpdateResult = post::Entity::update_many()
        .col(post::Column::Priority, post::Priority::High)
        .filter(post::Column::Id.eq(second.id))
        .exec(&db)
        .await
        .expect("update");
    assert_eq!(bumped.rows_affected, 1);

    let removed_one = post::Entity::delete(post::ActiveModel::from(second.clone()))
        .exec_with_returning(&db)
        .await
        .expect("delete one returning");
    assert_eq!(removed_one.id, second.id);
    assert_eq!(removed_one.priority, post::Priority::High);
    let removed_many = post::Entity::delete_many()
        .exec_with_returning(&db)
        .await
        .expect("delete many returning");
    assert_eq!(removed_many.len(), 1);
    assert_eq!(removed_many[0].id, created.id);
    assert_eq!(post::Entity::find().count(&db).await.expect("count"), 0);
}

/// A cursor pages forward and backward over a single or composite key, and offset pagination walks every page.
#[tokio::test]
async fn cursor_and_page_walking() {
    let db = setup().await;
    let alice = user(&db, "alice").await;
    let bob = user(&db, "bob").await;
    for (author, title) in [
        (alice.id, "a1"),
        (bob.id, "b1"),
        (alice.id, "a2"),
        (bob.id, "b2"),
        (alice.id, "a3"),
    ] {
        post(&db, author, title).await;
    }
    let ids = |rows: Vec<post::Model>| rows.into_iter().map(|p| p.id).collect::<Vec<_>>();

    let first = post::Entity::find()
        .cursor_by([post::Column::Id])
        .first(2)
        .all(&db)
        .await
        .expect("first");
    assert_eq!(ids(first), [1, 2]);
    let after = post::Entity::find()
        .cursor_by([post::Column::Id])
        .after(2)
        .first(2)
        .all(&db)
        .await
        .expect("after");
    assert_eq!(ids(after), [3, 4]);
    let last = post::Entity::find()
        .cursor_by([post::Column::Id])
        .before(5)
        .last(2)
        .all(&db)
        .await
        .expect("last");
    assert_eq!(ids(last), [3, 4]);
    let desc = post::Entity::find()
        .cursor_by([post::Column::Id])
        .desc()
        .after(4)
        .first(2)
        .all(&db)
        .await
        .expect("desc");
    assert_eq!(ids(desc), [3, 2]);

    // The composite key orders by author first, then id.
    let composite = post::Entity::find()
        .cursor_by([post::Column::AuthorId, post::Column::Id])
        .after((alice.id, 1))
        .first(3)
        .all(&db)
        .await
        .expect("composite");
    assert_eq!(ids(composite), [3, 5, 2]);

    let mut seen = Vec::new();
    post::Entity::find()
        .order_by_asc(post::Column::Id)
        .paginate(&db, 2)
        .for_each_page(|rows| {
            seen.extend(rows.into_iter().map(|p| p.id));
            true
        })
        .await
        .expect("walk");
    assert_eq!(seen, [1, 2, 3, 4, 5]);
}

/// A four-column composite key round-trips through `find_by_id` and `delete_by_id` as a tuple.
#[tokio::test]
async fn wide_composite_key() {
    let db = setup().await;
    quad::ActiveModel {
        a: Set(1),
        b: Set(2),
        c: Set("three".into()),
        d: Set(4),
        note: Set(Some("n".into())),
    }
    .insert(&db)
    .await
    .expect("insert");
    let found = quad::Entity::find_by_id((1, 2, "three".to_owned(), 4))
        .one(&db)
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(found.note.as_deref(), Some("n"));
    let deleted = quad::Entity::delete_by_id((1, 2, "three".to_owned(), 4))
        .exec(&db)
        .await
        .expect("delete");
    assert_eq!(deleted.rows_affected, 1);
}
