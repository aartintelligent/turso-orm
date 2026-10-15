//! The relational shapes: the many-to-many junction, the self-reference and
//! the second relation to one table, chains across tables, and cascades.
//!
//! `seed` writes the threads, records and tools the walkthrough reads; the
//! other functions run after the query walkthrough.

use entity::prelude::{Agent, Thread, ThreadRecord, ThreadTool, Tool};
use entity::thread_record::Role;
use entity::{agent, thread, thread_record, thread_tool, tool};
use turso_orm::entity::LoaderTrait;
use turso_orm::prelude::*;

/// Runs every relations example in sequence.
pub async fn all_about_relations(db: &Database) -> Result<(), DbErr> {
    many_to_many(db).await?;
    println!("----- -----\n");
    self_reference(db).await?;
    println!("----- -----\n");
    linked(db).await?;
    println!("----- -----\n");
    cascade(db).await?;
    Ok(())
}

/// The agent with the given name, which the mutation walkthrough inserted.
async fn agent_named(db: &Database, name: &str) -> Result<agent::Model, DbErr> {
    Ok(Agent::find()
        .filter(agent::Column::Name.eq(name))
        .one(db)
        .await?
        .expect("seeded by the mutation walkthrough"))
}

/// The thread with the given title, which `seed` inserted.
async fn thread_titled(db: &Database, title: &str) -> Result<thread::Model, DbErr> {
    Ok(Thread::find()
        .filter(thread::Column::Title.eq(title))
        .one(db)
        .await?
        .expect("seeded"))
}

/// Inserts three threads (one forked from another and reviewed), their
/// records, two tools and the junction rows between them.
pub async fn seed(db: &Database) -> Result<(), DbErr> {
    let planner = agent_named(db, "planner").await?;
    let writer = agent_named(db, "writer").await?;
    let root = thread::ActiveModel {
        agent_id: Set(planner.id),
        title: Set("Plan the launch".to_owned()),
        status: Set(thread::Status::Completed),
        ..Default::default()
    }
    .insert(db)
    .await?;
    let fork = thread::ActiveModel {
        agent_id: Set(writer.id),
        reviewer_agent_id: Set(Some(planner.id)),
        parent_id: Set(Some(root.id)),
        title: Set("Draft the announcement".to_owned()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    thread::ActiveModel {
        agent_id: Set(planner.id),
        title: Set("Collect the metrics".to_owned()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    ThreadRecord::insert_many([
        thread_record::ActiveModel {
            thread_id: Set(root.id),
            seq: Set(1),
            role: Set(Role::User),
            content: Set(Some("Plan the launch of the new model.".to_owned())),
            ..Default::default()
        },
        thread_record::ActiveModel {
            thread_id: Set(root.id),
            seq: Set(2),
            role: Set(Role::Assistant),
            tool_calls: Set(Some(serde_json::json!([
                { "name": "search", "arguments": { "query": "launch checklist" } }
            ]))),
            prompt_tokens: Set(Some(42)),
            completion_tokens: Set(Some(17)),
            ..Default::default()
        },
        thread_record::ActiveModel {
            thread_id: Set(fork.id),
            seq: Set(1),
            role: Set(Role::User),
            content: Set(Some("Draft the announcement from the plan.".to_owned())),
            ..Default::default()
        },
    ])
    .exec(db)
    .await?;
    let search = tool::ActiveModel {
        name: Set("search".to_owned()),
        description: Set(Some("Searches the web.".to_owned())),
        ..Default::default()
    }
    .insert(db)
    .await?;
    let calculator = tool::ActiveModel {
        name: Set("calculator".to_owned()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    for (thread_id, tool_id) in [
        (root.id, search.id),
        (root.id, calculator.id),
        (fork.id, search.id),
    ] {
        thread_tool::ActiveModel {
            thread_id: Set(thread_id),
            tool_id: Set(tool_id),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

// --8<-- [start:linked]
/// The chain from an agent to the tools its threads invoked: three hops,
/// each an existing relation definition.
struct AgentToTool;

impl Linked for AgentToTool {
    type FromEntity = agent::Entity;
    type ToEntity = tool::Entity;

    fn link(&self) -> Vec<RelationDef> {
        vec![
            agent::Relation::Thread.def(),
            thread::Relation::ThreadTool.def(),
            thread_tool::Relation::Tool.def(),
        ]
    }
}
// --8<-- [end:linked]

// --8<-- [start:many_to_many]
/// Walks the many-to-many relation in every form: from one model, as a
/// join, as pairs, grouped, and batch-loaded for a list of threads.
async fn many_to_many(db: &Database) -> Result<(), DbErr> {
    let root = thread_titled(db, "Plan the launch").await?;

    // From one model, through the junction, like any other relation.
    let tools = root.find_related(Tool).all(db).await?;
    println!("tools of {:?}: {:?}", root.title, tools);

    // A join through the junction lets a filter mention the other side.
    let searched = Thread::find()
        .inner_join(Tool)
        .filter(tool::Column::Name.eq("search"))
        .all(db)
        .await?;
    println!("threads that searched: {}", searched.len());

    // One query, one entry per thread, with all of its tools.
    let grouped = Thread::find()
        .find_with_related(Tool)
        .order_by(thread::Column::Id, Order::Asc)
        .all(db)
        .await?;
    for (thread, tools) in &grouped {
        println!(
            "{} -> {:?}",
            thread.title,
            tools.iter().map(|t| &t.name).collect::<Vec<_>>()
        );
    }

    // The loader takes a list of threads and answers in input order.
    let threads = Thread::find().all(db).await?;
    let per_thread = threads.load_many(Tool, db).await?;
    for (thread, tools) in threads.iter().zip(&per_thread) {
        println!("loaded {}: {} tool(s)", thread.title, tools.len());
    }
    Ok(())
}
// --8<-- [end:many_to_many]

// --8<-- [start:self_reference]
/// Follows the self-reference and the second relation to `agent`.
async fn self_reference(db: &Database) -> Result<(), DbErr> {
    let fork = thread_titled(db, "Draft the announcement").await?;

    // `Related<thread::Entity>` is `Parent`, the first self-relation declared.
    let parent = fork.find_related(Thread).one(db).await?;
    println!("parent of {:?}: {:?}", fork.title, parent.map(|t| t.title));

    // The reverse side goes through its definition.
    let root = thread_titled(db, "Plan the launch").await?;
    let forks = Thread::find()
        .related_to(&thread::Relation::Children.def(), &root)
        .all(db)
        .await?;
    println!("forks of {:?}: {}", root.title, forks.len());

    // A second relation to the same entity is joined under an alias, so a
    // filter can name it.
    let reviewed_by_planner = Thread::find()
        .join_as(
            JoinType::Inner,
            &thread::Relation::Reviewer.def(),
            "reviewer",
        )
        .filter(Expr::col(("reviewer", "name")).eq("planner"))
        .all(db)
        .await?;
    println!("threads reviewed by planner: {}", reviewed_by_planner.len());
    Ok(())
}
// --8<-- [end:self_reference]

// --8<-- [start:find_linked]
/// Reaches the tools of an agent's threads in one query through the chain.
async fn linked(db: &Database) -> Result<(), DbErr> {
    let planner = agent_named(db, "planner").await?;
    let tools = planner.find_linked(AgentToTool).distinct().all(db).await?;
    println!(
        "tools used by planner: {:?}",
        tools.iter().map(|t| &t.name).collect::<Vec<_>>()
    );

    let per_agent = Agent::find()
        .find_with_linked(&AgentToTool)
        .order_by(agent::Column::Id, Order::Asc)
        .all(db)
        .await?;
    for (agent, tools) in &per_agent {
        println!("{} -> {} tool row(s)", agent.name, tools.len());
    }
    Ok(())
}
// --8<-- [end:find_linked]

// --8<-- [start:cascade]
/// Deletes an agent and lets the engine cascade to its threads, their
/// records and their junction rows.
async fn cascade(db: &Database) -> Result<(), DbErr> {
    let before = Thread::find().count(db).await?;
    let planner = agent_named(db, "planner").await?;
    let owned = planner.find_related(Thread).count(db).await?;
    planner.delete(db).await?;
    let after = Thread::find().count(db).await?;
    println!("threads: {before} before, {owned} owned by planner, {after} after the cascade");
    println!(
        "junction rows left: {}",
        ThreadTool::find().count(db).await?
    );
    Ok(())
}
// --8<-- [end:cascade]
