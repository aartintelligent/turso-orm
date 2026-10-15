//! Reads: finders, filters, ordering, pagination, streaming, projections,
//! the cursor and relations in both directions.
//!
//! Every function prints the models it reads so that the console output
//! mirrors the code.

use entity::prelude::{Agent, LlmModel, LlmProvider, Thread};
use entity::{agent, llm_model, thread};
use futures_util::TryStreamExt;
use turso_orm::prelude::*;

/// Runs every query example in sequence.
pub async fn all_about_query(db: &Database) -> Result<(), DbErr> {
    find_all(db).await?;
    println!("----- -----\n");
    find_one_and_filter(db).await?;
    println!("----- -----\n");
    paginate_and_stream(db).await?;
    println!("----- -----\n");
    cursor(db).await?;
    println!("----- -----\n");
    project_into_struct(db).await?;
    println!("----- -----\n");
    projections(db).await?;
    println!("----- -----\n");
    relations(db).await?;
    println!("----- -----\n");
    loaders(db).await?;
    Ok(())
}

// --8<-- [start:find_all]
/// Lists every row of two tables.
async fn find_all(db: &Database) -> Result<(), DbErr> {
    println!("find all models:");
    for model in LlmModel::find().all(db).await? {
        println!("  {model:?}");
    }
    println!("find all agents:");
    for agent in Agent::find().all(db).await? {
        println!("  {agent:?}");
    }
    Ok(())
}
// --8<-- [end:find_all]

// --8<-- [start:filter]
/// Finds by key, by `LIKE`, and with a composed `OR` condition.
async fn find_one_and_filter(db: &Database) -> Result<(), DbErr> {
    let by_id = LlmModel::find_by_id(1).one(db).await?;
    println!("find one by primary key: {by_id:?}");

    let by_name = LlmModel::find()
        .filter(llm_model::Column::Name.contains("mini"))
        .one(db)
        .await?;
    println!("find one by name: {by_name:?}");

    let cond = Condition::any()
        .add(llm_model::Column::SupportsTools.eq(true))
        .add(llm_model::Column::InputPrice.gt(2.0));
    let models = LlmModel::find()
        .filter(cond)
        .order_by_desc(llm_model::Column::InputPrice)
        .all(db)
        .await?;
    println!("tool-capable or pricey, most expensive first:");
    for model in models {
        println!("  {model:?}");
    }
    Ok(())
}
// --8<-- [end:filter]

// --8<-- [start:paginate_stream]
/// Pages through agents two at a time, then streams them one row at a time.
async fn paginate_and_stream(db: &Database) -> Result<(), DbErr> {
    let paginator = Agent::find()
        .order_by_asc(agent::Column::Id)
        .paginate(db, 2);
    let (items, pages) = paginator.num_items_and_pages().await?;
    println!("agents: {items} items over {pages} pages");
    for page in 0..pages {
        println!("  page {page}: {:?}", paginator.fetch_page(page).await?);
    }

    println!("stream of agent names:");
    let mut stream = Agent::find()
        .order_by_asc(agent::Column::Name)
        .stream(db)
        .await?;
    while let Some(agent) = stream.try_next().await? {
        println!("  {}", agent.name);
    }
    Ok(())
}
// --8<-- [end:paginate_stream]

// --8<-- [start:cursor]
/// Pages through threads by key, two at a time, the way an API with
/// `?after=<id>` would.
async fn cursor(db: &Database) -> Result<(), DbErr> {
    let mut after: Option<i32> = None;
    loop {
        let mut page = Thread::find().cursor_by([thread::Column::Id]).first(2);
        if let Some(id) = after {
            page = page.after(id);
        }
        let rows = page.all(db).await?;
        println!("page: {:?}", rows.iter().map(|t| t.id).collect::<Vec<_>>());
        match rows.last() {
            Some(last) if rows.len() == 2 => after = Some(last.id),
            _ => return Ok(()),
        }
    }
}
// --8<-- [end:cursor]

// --8<-- [start:project_struct]
/// Aggregates over the models decoded into a plain struct.
#[derive(Debug, FromQueryResult)]
struct ModelStats {
    /// Number of models.
    count: i64,
    /// Highest price per million input tokens.
    #[turso(column_name = "max_price")]
    most_expensive: f64,
}
// --8<-- [end:project_struct]

// --8<-- [start:project]
/// Selects only computed columns and decodes them by name.
async fn project_into_struct(db: &Database) -> Result<(), DbErr> {
    let stats = LlmModel::find()
        .select_only()
        .expr_as(Func::count_star(), "count")
        .expr_as(llm_model::Column::InputPrice.max(), "max_price")
        .into_model::<ModelStats>()
        .one(db)
        .await?;
    if let Some(ModelStats {
        count,
        most_expensive,
    }) = stats
    {
        println!("{count} models, the most expensive at {most_expensive} per million tokens");
    }
    Ok(())
}
// --8<-- [end:project]

// --8<-- [start:partial_model]
/// A projection of `thread`: two columns under their own names and one
/// expression, selected and decoded from the one declaration.
#[derive(Debug, DerivePartialModel)]
#[turso(entity = "Thread")]
struct ThreadSummary {
    id: i32,
    #[turso(from_col = "Title")]
    text: String,
    #[turso(from_expr = "Expr::col((\"thread\", \"title\")).concat(\" (open)\")")]
    open_text: String,
}
// --8<-- [end:partial_model]

// --8<-- [start:projections]
/// Reads threads as a partial model, as tuples and as JSON, and probes with `exists`.
async fn projections(db: &Database) -> Result<(), DbErr> {
    let summaries = Thread::find()
        .filter(thread::Column::Status.eq(thread::Status::Open))
        .into_partial_model::<ThreadSummary>()
        .all(db)
        .await?;
    for s in &summaries {
        println!("summary {}: {:?} / {:?}", s.id, s.text, s.open_text);
    }

    let pairs = Thread::find()
        .select_only()
        .column(thread::Column::Id)
        .column_as(thread::Column::Title, "title")
        .into_tuple::<(i32, String)>()
        .all(db)
        .await?;
    println!("pairs: {pairs:?}");

    let json = Thread::find().into_json().one(db).await?;
    println!("as json: {json:?}");

    let any_completed = Thread::find()
        .filter(thread::Column::Status.eq(thread::Status::Completed))
        .exists(db)
        .await?;
    println!("any completed thread: {any_completed}");
    Ok(())
}
// --8<-- [end:projections]

// --8<-- [start:relations]
/// Walks relations in both directions, with one query per hop or a single join.
async fn relations(db: &Database) -> Result<(), DbErr> {
    let provider = LlmProvider::find().one(db).await?.expect("seeded");
    println!("models served by {}:", provider.name);
    for model in provider.find_related(LlmModel).all(db).await? {
        println!("  {}", model.name);
    }

    let agent = Agent::find()
        .order_by_asc(agent::Column::Id)
        .one(db)
        .await?
        .expect("seeded");
    let model = agent.find_related(LlmModel).one(db).await?;
    println!("{} runs on {:?}", agent.name, model.map(|m| m.name));

    println!("every agent with its model in one SELECT:");
    for (agent, model) in Agent::find()
        .find_also_related(LlmModel)
        .order_by(agent::Column::Id, Order::Asc)
        .all(db)
        .await?
    {
        println!("  {} -> {:?}", agent.name, model.map(|m| m.name));
    }

    println!("agents on models above 1.0 (inner join, filter on the joined table):");
    for agent in Agent::find()
        .inner_join(LlmModel)
        .filter(llm_model::Column::InputPrice.gt(1.0))
        .all(db)
        .await?
    {
        println!("  {}", agent.name);
    }
    Ok(())
}
// --8<-- [end:relations]

// --8<-- [start:loaders]
/// Batch loaders: one `IN (...)` query for many parents, results aligned with the input.
async fn loaders(db: &Database) -> Result<(), DbErr> {
    let models = LlmModel::find()
        .order_by_asc(llm_model::Column::Id)
        .all(db)
        .await?;
    let agents = models.load_many(Agent, db).await?;
    println!("agents per model through load_many:");
    for (model, agents) in models.iter().zip(&agents) {
        let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
        println!("  {}: {names:?}", model.name);
    }

    let providers = models.load_one(LlmProvider, db).await?;
    println!("provider per model through load_one:");
    for (model, provider) in models.iter().zip(&providers) {
        println!(
            "  {} <- {:?}",
            model.name,
            provider.as_ref().map(|p| &p.name)
        );
    }
    Ok(())
}
// --8<-- [end:loaders]
