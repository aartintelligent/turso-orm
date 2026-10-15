//! The threads: opening, paging by key, appending records, forking, tools.

use entity::prelude::{Thread, ThreadRecord, Tool};
use entity::{thread, thread_record, thread_tool, tool};
use turso_orm::prelude::*;

use crate::dto::thread::{
    AppendRecord, OpenThread, ThreadSummary, ThreadWithRecords, ThreadWithTools,
};

/// Opens a thread from the request body.
pub async fn open_thread(db: &Database, body: OpenThread) -> Result<thread::Model, DbErr> {
    body.into_active_model().insert(db).await
}

// --8<-- [start:requests]
/// Opens a thread from a raw JSON object: every key that names a column is
/// set, the others are ignored.
pub async fn open_thread_from_json(
    db: &Database,
    body: serde_json::Value,
) -> Result<thread::Model, DbErr> {
    thread::ActiveModel::from_json(body)?.insert(db).await
}

/// Deletes the threads whose title starts with `prefix` and returns them as they were.
pub async fn purge_threads(db: &Database, prefix: &str) -> Result<Vec<thread::Model>, DbErr> {
    Thread::delete_many()
        .filter(thread::Column::Title.starts_with(prefix))
        .exec_with_returning(db)
        .await
}
// --8<-- [end:requests]

// --8<-- [start:cursor]
/// The threads after a key, `size` at a time, the way an API with
/// `?after=<id>` pages: an index seek that stays stable under writes.
pub async fn threads_after(
    db: &Database,
    after: Option<i32>,
    size: u64,
) -> Result<Vec<thread::Model>, DbErr> {
    let mut page = Thread::find()
        .cursor_by([thread::Column::Id])
        .first(size.max(1));
    if let Some(id) = after {
        page = page.after(id);
    }
    page.all(db).await
}
// --8<-- [end:cursor]

/// A thread with its records in order; `None` when the thread does not exist.
pub async fn thread_with_records(
    db: &Database,
    id: i32,
) -> Result<Option<ThreadWithRecords>, DbErr> {
    let groups = Thread::find_by_id(id)
        .find_with_related(ThreadRecord)
        .order_by_related(thread_record::Column::Seq, Order::Asc)
        .all(db)
        .await?;
    Ok(groups
        .into_iter()
        .next()
        .map(|(thread, records)| ThreadWithRecords { thread, records }))
}

// --8<-- [start:transaction]
/// Appends records to a thread, numbered after the last one, in one transaction:
/// the sequence read and the insert either both happen or neither does.
pub async fn append_records(
    db: &Database,
    thread_id: i32,
    bodies: Vec<AppendRecord>,
) -> Result<Vec<thread_record::Model>, DbErr> {
    db.transaction(|txn| {
        Box::pin(async move {
            let next = ThreadRecord::find()
                .filter(thread_record::Column::ThreadId.eq(thread_id))
                .count(txn)
                .await?;
            let next = i32::try_from(next).map_err(|e| DbErr::Custom(e.to_string()))? + 1;
            let records = bodies.into_iter().enumerate().map(|(i, body)| {
                let seq = i32::try_from(i).map_or(i32::MAX, |i| next + i);
                thread_record::ActiveModel {
                    thread_id: Set(thread_id),
                    seq: Set(seq),
                    role: Set(body.role),
                    content: Set(body.content),
                    tool_calls: Set(body.tool_calls),
                    ..Default::default()
                }
            });
            ThreadRecord::insert_many(records)
                .exec_with_returning(txn)
                .await
        })
    })
    .await
}
// --8<-- [end:transaction]

// --8<-- [start:self_reference]
/// Opens a thread forked from another: the self-reference is a plain column.
pub async fn fork_thread(
    db: &Database,
    parent: &thread::Model,
    agent_id: i32,
) -> Result<thread::Model, DbErr> {
    thread::ActiveModel {
        agent_id: Set(agent_id),
        parent_id: Set(Some(parent.id)),
        title: Set(format!("Fork of {}", parent.title)),
        ..Default::default()
    }
    .insert(db)
    .await
}

/// The thread this one was forked from: `Related<thread::Entity>` is
/// `Parent`, the first self-relation declared.
pub async fn parent_of(
    db: &Database,
    fork: &thread::Model,
) -> Result<Option<thread::Model>, DbErr> {
    fork.find_related(Thread).one(db).await
}

/// The threads forked from this one: the reverse side, through its definition.
pub async fn forks_of(db: &Database, root: &thread::Model) -> Result<Vec<thread::Model>, DbErr> {
    Thread::find()
        .related_to(&thread::Relation::Children.def(), root)
        .all(db)
        .await
}

/// The threads an agent reviews: the second relation to `agent`, joined
/// under an alias so that the filter can name it.
pub async fn reviewed_by(db: &Database, agent_name: &str) -> Result<Vec<thread::Model>, DbErr> {
    Thread::find()
        .join_as(
            JoinType::Inner,
            &thread::Relation::Reviewer.def(),
            "reviewer",
        )
        .filter(Expr::col(("reviewer", "name")).eq(agent_name))
        .all(db)
        .await
}
// --8<-- [end:self_reference]

/// Records that a thread invoked a tool, creating the tool on first sight.
pub async fn record_tool_use(
    db: &Database,
    thread_id: i32,
    name: &str,
) -> Result<tool::Model, DbErr> {
    let tool = match Tool::find()
        .filter(tool::Column::Name.eq(name))
        .one(db)
        .await?
    {
        Some(tool) => tool,
        None => {
            tool::ActiveModel {
                name: Set(name.to_owned()),
                ..Default::default()
            }
            .insert(db)
            .await?
        }
    };
    thread_tool::ActiveModel {
        thread_id: Set(thread_id),
        tool_id: Set(tool.id),
    }
    .insert(db)
    .await?;
    Ok(tool)
}

// --8<-- [start:many_to_many]
/// The tools of one thread: from one model, through the junction, like any
/// other relation.
pub async fn tools_of_thread(
    db: &Database,
    thread: &thread::Model,
) -> Result<Vec<tool::Model>, DbErr> {
    thread.find_related(Tool).all(db).await
}

/// The threads that invoked a tool: a join through the junction lets the
/// filter mention the other side.
pub async fn threads_using(db: &Database, tool_name: &str) -> Result<Vec<thread::Model>, DbErr> {
    Thread::find()
        .inner_join(Tool)
        .filter(tool::Column::Name.eq(tool_name))
        .all(db)
        .await
}

/// Every thread with all of its tools: one query, one entry per thread.
pub async fn threads_with_tools(db: &Database) -> Result<Vec<ThreadWithTools>, DbErr> {
    let groups = Thread::find()
        .find_with_related(Tool)
        .order_by(thread::Column::Id, Order::Asc)
        .all(db)
        .await?;
    Ok(groups
        .into_iter()
        .map(|(thread, tools)| ThreadWithTools { thread, tools })
        .collect())
}

/// The tools of each thread of a list, in input order, through the loader.
pub async fn tools_for(
    db: &Database,
    threads: &[thread::Model],
) -> Result<Vec<Vec<tool::Model>>, DbErr> {
    threads.load_many(Tool, db).await
}
// --8<-- [end:many_to_many]

// --8<-- [start:projections]
/// The open threads as a partial model that selects its own columns.
pub async fn summaries(db: &Database) -> Result<Vec<ThreadSummary>, DbErr> {
    Thread::find()
        .filter(thread::Column::Status.eq(thread::Status::Open))
        .into_partial_model::<ThreadSummary>()
        .all(db)
        .await
}

/// Ids and titles as tuples, decoded by position.
pub async fn titles(db: &Database) -> Result<Vec<(i32, String)>, DbErr> {
    Thread::find()
        .select_only()
        .column(thread::Column::Id)
        .column_as(thread::Column::Title, "title")
        .into_tuple::<(i32, String)>()
        .all(db)
        .await
}

/// The first thread as a JSON object keyed by column name.
pub async fn first_as_json(db: &Database) -> Result<Option<serde_json::Value>, DbErr> {
    Thread::find().into_json().one(db).await
}

/// Whether at least one thread is completed, without reading any.
pub async fn any_completed(db: &Database) -> Result<bool, DbErr> {
    Thread::find()
        .filter(thread::Column::Status.eq(thread::Status::Completed))
        .exists(db)
        .await
}
// --8<-- [end:projections]
