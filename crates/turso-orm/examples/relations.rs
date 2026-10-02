//! A blog schema exercising the relational features: many-to-many through a junction, a self-reference, two relations to one entity, a multi-hop chain, enum columns, partial models, request structs, JSON input and keyset pagination.
//!
//! The schema is `author`, `post` (written by an author, optionally edited
//! by another, optionally replying to a parent post) and `tag`, with
//! `post_tag` as the junction between posts and tags. Everything runs
//! against an in-memory database, so the example needs no setup.
//!
//! ```text
//! cargo run -p turso-orm --example relations
//! ```

#![allow(clippy::print_stdout, reason = "examples are meant to print")]

/// The `author` entity.
mod author {
    use turso_orm::prelude::*;

    /// A row of the `author` table.
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[turso(table_name = "author")]
    pub(crate) struct Model {
        #[turso(primary_key)]
        pub id: i32,
        #[turso(unique)]
        pub name: String,
    }

    /// The posts an author wrote; the columns come from the first relation
    /// `post` declares back to `author`.
    #[derive(Copy, Clone, Debug, DeriveRelation)]
    pub(crate) enum Relation {
        #[turso(has_many = "super::post::Entity")]
        Posts,
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// The `post` entity.
mod post {
    use turso_orm::prelude::*;

    // --8<-- [start:active_enum]
    /// The publication state of a post, stored as text.
    ///
    /// Without `string_value`, a variant is stored as its `snake_case` name.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, DeriveActiveEnum)]
    #[turso(rs_type = "String")]
    pub(crate) enum Status {
        Draft,
        #[turso(string_value = "live")]
        Published,
    }
    // --8<-- [end:active_enum]

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
    }

    // --8<-- [start:relations]
    /// The relations of `post`.
    ///
    /// `Author` and `Editor` both point at `author`: the first one provides
    /// `Related<author::Entity>`, the second is used through its `def()`.
    /// `Parent` is the self-reference and `Children` its inferred reverse.
    /// `Tags` goes through the `post_tag` junction.
    #[derive(Copy, Clone, Debug, DeriveRelation)]
    pub(crate) enum Relation {
        #[turso(
            belongs_to = "super::author::Entity",
            from = "Column::AuthorId",
            to = "super::author::Column::Id",
            on_delete = "Cascade"
        )]
        Author,
        #[turso(
            belongs_to = "super::author::Entity",
            from = "Column::EditorId",
            to = "super::author::Column::Id",
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
        #[turso(has_many = "super::post_tag::Entity")]
        PostTag,
        #[turso(has_many = "super::tag::Entity", via = "super::post_tag::Entity")]
        Tags,
    }
    // --8<-- [end:relations]

    impl ActiveModelBehavior for ActiveModel {}
}

/// The `tag` entity.
mod tag {
    use turso_orm::prelude::*;

    /// A row of the `tag` table.
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[turso(table_name = "tag")]
    pub(crate) struct Model {
        #[turso(primary_key)]
        pub id: i32,
        #[turso(unique)]
        pub label: String,
    }

    /// The relations of `tag`: the junction rows, and the posts through them.
    #[derive(Copy, Clone, Debug, DeriveRelation)]
    pub(crate) enum Relation {
        #[turso(has_many = "super::post_tag::Entity")]
        PostTag,
        #[turso(has_many = "super::post::Entity", via = "super::post_tag::Entity")]
        Posts,
    }

    impl ActiveModelBehavior for ActiveModel {}
}

// --8<-- [start:junction]
/// The junction between `post` and `tag`: a composite key and a
/// `belongs_to` towards each side, which is all a many-to-many relation
/// needs.
mod post_tag {
    use turso_orm::prelude::*;

    /// A row of the `post_tag` table.
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[turso(table_name = "post_tag")]
    pub(crate) struct Model {
        #[turso(primary_key)]
        pub post_id: i32,
        #[turso(primary_key)]
        pub tag_id: i32,
    }

    /// The two sides of the junction.
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
            belongs_to = "super::tag::Entity",
            from = "Column::TagId",
            to = "super::tag::Column::Id",
            on_delete = "Cascade"
        )]
        Tag,
    }

    impl ActiveModelBehavior for ActiveModel {}
}
// --8<-- [end:junction]

use turso_orm::entity::LoaderTrait;
use turso_orm::prelude::*;

// --8<-- [start:linked]
/// The chain from an author to the tags of the posts they wrote: three
/// hops, each an existing relation definition.
struct AuthorToTag;

impl Linked for AuthorToTag {
    type FromEntity = author::Entity;
    type ToEntity = tag::Entity;

    fn link(&self) -> Vec<RelationDef> {
        vec![
            author::Relation::Posts.def(),
            post::Relation::PostTag.def(),
            post_tag::Relation::Tag.def(),
        ]
    }
}
// --8<-- [end:linked]

// --8<-- [start:partial_model]
/// A projection of `post`: two columns under their own names and one
/// expression, selected and decoded from the one declaration.
#[derive(Debug, DerivePartialModel)]
#[turso(entity = "post::Entity")]
struct Headline {
    id: i32,
    #[turso(from_col = "Title")]
    text: String,
    #[turso(from_expr = "Expr::col((\"post\", \"title\")).concat(\" (draft)\")")]
    draft_text: String,
}
// --8<-- [end:partial_model]

// --8<-- [start:into_active_model]
/// The body of a "create post" request: a subset of the columns, with an
/// optional editor that is only set when the client sends one.
#[derive(Debug, DeriveIntoActiveModel)]
#[turso(active_model = "post::ActiveModel")]
struct CreatePost {
    author_id: i32,
    title: String,
    editor_id: Option<i32>,
}
// --8<-- [end:into_active_model]

/// Creates every table and index of the schema.
async fn create_schema(db: &Database) -> Result<(), DbErr> {
    let schema = Schema::new();
    for stmt in [
        schema.create_table_from_entity(author::Entity),
        schema.create_table_from_entity(post::Entity),
        schema.create_table_from_entity(tag::Entity),
        schema.create_table_from_entity(post_tag::Entity),
    ] {
        db.execute(stmt.to_statement()).await?;
    }
    for idx in schema.create_index_from_entity(post::Entity) {
        db.execute(idx.to_statement()).await?;
    }
    Ok(())
}

/// Inserts two authors, three posts (one replying to another), two tags
/// and the junction rows between them.
async fn seed(db: &Database) -> Result<(), DbErr> {
    let ada = author::ActiveModel {
        name: Set("ada".into()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    let bob = author::ActiveModel {
        name: Set("bob".into()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    let root = post::ActiveModel {
        author_id: Set(ada.id),
        title: Set("Hello, Turso".into()),
        status: Set(post::Status::Published),
        ..Default::default()
    }
    .insert(db)
    .await?;
    let reply = post::ActiveModel {
        author_id: Set(bob.id),
        editor_id: Set(Some(ada.id)),
        parent_id: Set(Some(root.id)),
        title: Set("Re: Hello, Turso".into()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    post::ActiveModel {
        author_id: Set(ada.id),
        title: Set("Vectors in SQL".into()),
        status: Set(post::Status::Published),
        ..Default::default()
    }
    .insert(db)
    .await?;
    let rust = tag::ActiveModel {
        label: Set("rust".into()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    let db_tag = tag::ActiveModel {
        label: Set("databases".into()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    for (post_id, tag_id) in [
        (root.id, rust.id),
        (root.id, db_tag.id),
        (reply.id, rust.id),
    ] {
        post_tag::ActiveModel {
            post_id: Set(post_id),
            tag_id: Set(tag_id),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

// --8<-- [start:many_to_many]
/// Walks the many-to-many relation in every form: from one model, as a
/// join, as pairs, grouped, and batch-loaded for a list of posts.
async fn many_to_many(db: &Database) -> Result<(), DbErr> {
    let root = post::Entity::find_by_id(1).one(db).await?.expect("seeded");

    // From one model, through the junction, like any other relation.
    let tags = root.find_related(tag::Entity).all(db).await?;
    println!("tags of {:?}: {:?}", root.title, tags);

    // A join through the junction lets a filter mention the other side.
    let about_rust = post::Entity::find()
        .inner_join(tag::Entity)
        .filter(tag::Column::Label.eq("rust"))
        .all(db)
        .await?;
    println!("posts tagged rust: {}", about_rust.len());

    // One query, one entry per post, with all of its tags.
    let grouped = post::Entity::find()
        .find_with_related(tag::Entity)
        .order_by(post::Column::Id, Order::Asc)
        .all(db)
        .await?;
    for (post, tags) in &grouped {
        println!(
            "{} -> {:?}",
            post.title,
            tags.iter().map(|t| &t.label).collect::<Vec<_>>()
        );
    }

    // The loader takes a list of posts and answers in input order.
    let posts = post::Entity::find().all(db).await?;
    let per_post = posts.load_many(tag::Entity, db).await?;
    for (post, tags) in posts.iter().zip(&per_post) {
        println!("loaded {}: {} tag(s)", post.title, tags.len());
    }
    Ok(())
}
// --8<-- [end:many_to_many]

// --8<-- [start:self_reference]
/// Follows the self-reference and the second relation to `author`.
async fn self_reference(db: &Database) -> Result<(), DbErr> {
    let reply = post::Entity::find_by_id(2).one(db).await?.expect("seeded");

    // `Related<post::Entity>` is `Parent`, the first self-relation declared.
    let parent = reply.find_related(post::Entity).one(db).await?;
    println!("parent of {:?}: {:?}", reply.title, parent.map(|p| p.title));

    // The reverse side goes through its definition.
    let root = post::Entity::find_by_id(1).one(db).await?.expect("seeded");
    let children = post::Entity::find()
        .related_to(&post::Relation::Children.def(), &root)
        .all(db)
        .await?;
    println!("replies to {:?}: {}", root.title, children.len());

    // A second relation to the same entity is joined under an alias, so a
    // filter can name it.
    let edited_by_ada = post::Entity::find()
        .join_as(JoinType::Inner, &post::Relation::Editor.def(), "editor")
        .filter(Expr::col(("editor", "name")).eq("ada"))
        .all(db)
        .await?;
    println!("posts edited by ada: {}", edited_by_ada.len());
    Ok(())
}
// --8<-- [end:self_reference]

// --8<-- [start:find_linked]
/// Reaches the tags of an author's posts in one query through the chain.
async fn linked(db: &Database) -> Result<(), DbErr> {
    let ada = author::Entity::find()
        .filter(author::Column::Name.eq("ada"))
        .one(db)
        .await?
        .expect("seeded");
    let tags = ada.find_linked(AuthorToTag).distinct().all(db).await?;
    println!(
        "tags used by ada: {:?}",
        tags.iter().map(|t| &t.label).collect::<Vec<_>>()
    );

    let per_author = author::Entity::find()
        .find_with_linked(&AuthorToTag)
        .order_by(author::Column::Id, Order::Asc)
        .all(db)
        .await?;
    for (author, tags) in &per_author {
        println!("{} -> {} tag row(s)", author.name, tags.len());
    }
    Ok(())
}
// --8<-- [end:find_linked]

// --8<-- [start:projections]
/// Reads posts as a partial model, as tuples and as JSON, and probes with `exists`.
async fn projections(db: &Database) -> Result<(), DbErr> {
    let headlines = post::Entity::find()
        .filter(post::Column::Status.eq(post::Status::Published))
        .into_partial_model::<Headline>()
        .all(db)
        .await?;
    for h in &headlines {
        println!("headline {}: {:?} / {:?}", h.id, h.text, h.draft_text);
    }

    let pairs = post::Entity::find()
        .select_only()
        .column(post::Column::Id)
        .column_as(post::Column::Title, "title")
        .into_tuple::<(i32, String)>()
        .all(db)
        .await?;
    println!("pairs: {pairs:?}");

    let json = post::Entity::find().into_json().one(db).await?;
    println!("as json: {json:?}");

    let any_draft = post::Entity::find()
        .filter(post::Column::Status.eq(post::Status::Draft))
        .exists(db)
        .await?;
    println!("any draft: {any_draft}");
    Ok(())
}
// --8<-- [end:projections]

// --8<-- [start:requests]
/// Creates posts from a request struct and from a JSON body, and deletes
/// one with `RETURNING`.
async fn requests(db: &Database) -> Result<(), DbErr> {
    let created = CreatePost {
        author_id: 2,
        title: "From a request".into(),
        editor_id: None,
    }
    .into_active_model()
    .insert(db)
    .await?;
    println!(
        "created {:?} with status {:?}",
        created.title, created.status
    );

    let from_json = post::ActiveModel::from_json(serde_json::json!({
        "author_id": 1,
        "title": "From JSON",
        "status": "live"
    }))?
    .insert(db)
    .await?;
    println!(
        "created {:?} with status {:?}",
        from_json.title, from_json.status
    );

    let removed = post::Entity::delete_many()
        .filter(post::Column::Title.starts_with("From "))
        .exec_with_returning(db)
        .await?;
    println!("removed {} post(s)", removed.len());
    Ok(())
}
// --8<-- [end:requests]

// --8<-- [start:cursor]
/// Pages through posts by key, two at a time, the way an API with
/// `?after=<id>` would.
async fn cursor(db: &Database) -> Result<(), DbErr> {
    let mut after: Option<i32> = None;
    loop {
        let mut page = post::Entity::find().cursor_by([post::Column::Id]).first(2);
        if let Some(id) = after {
            page = page.after(id);
        }
        let rows = page.all(db).await?;
        println!("page: {:?}", rows.iter().map(|p| p.id).collect::<Vec<_>>());
        match rows.last() {
            Some(last) if rows.len() == 2 => after = Some(last.id),
            _ => return Ok(()),
        }
    }
}
// --8<-- [end:cursor]

/// Creates the schema, seeds it and runs every section in turn.
#[tokio::main]
async fn main() -> Result<(), DbErr> {
    let db = Database::connect(ConnectOptions::in_memory()).await?;
    create_schema(&db).await?;
    seed(&db).await?;
    many_to_many(&db).await?;
    self_reference(&db).await?;
    linked(&db).await?;
    projections(&db).await?;
    requests(&db).await?;
    cursor(&db).await?;
    Ok(())
}
