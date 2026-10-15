//! The threads: opening, paging by key, completing, forking, the reviewer,
//! and the projections of the table.

use crate::entity::thread_record::Role;
use crate::entity::{Thread, ThreadRecord, Tool};
use crate::entity::{thread, thread_record, thread_tool, tool};
use turso_orm::prelude::*;
use turso_orm::{ConnectionTrait, TransactionTrait};

use crate::dto::thread::{
    OpenThread, OpenWithTools, ThreadSummary, ThreadWithRecords, ThreadWithTools,
};

/// Opens a thread from the request body.
pub async fn open_thread<C: ConnectionTrait>(
    db: &C,
    body: OpenThread,
) -> Result<thread::Model, DbErr> {
    body.into_active_model().insert(db).await
}

// --8<-- [start:requests]
/// Opens a thread from a raw JSON object: every key that names a column is
/// set, the others are ignored.
pub async fn open_thread_from_json<C: ConnectionTrait>(
    db: &C,
    body: serde_json::Value,
) -> Result<thread::Model, DbErr> {
    thread::ActiveModel::from_json(body)?.insert(db).await
}

/// Deletes the threads whose title starts with `prefix` and returns them as they were.
pub async fn purge_threads<C: ConnectionTrait>(
    db: &C,
    prefix: &str,
) -> Result<Vec<thread::Model>, DbErr> {
    Thread::delete_many()
        .filter(thread::Column::Title.starts_with(prefix))
        .exec_with_returning(db)
        .await
}
// --8<-- [end:requests]

// --8<-- [start:cursor]
/// The threads after a key, `size` at a time, the way an API with
/// `?after=<id>` pages: an index seek that stays stable under writes.
pub async fn threads_after<C: ConnectionTrait>(
    db: &C,
    after: Option<i32>,
    size: u64,
) -> Result<Vec<thread::Model>, DbErr> {
    let mut page = Thread::find().cursor_by([thread::Column::Id]).first(size);
    if let Some(id) = after {
        page = page.after(id);
    }
    page.all(db).await
}
// --8<-- [end:cursor]

/// A thread with its records in order; `None` when the thread does not exist.
pub async fn thread_with_records<C: ConnectionTrait>(
    db: &C,
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

// --8<-- [start:begin_commit]
/// Completes a thread: the status change and the closing record go through
/// one explicit transaction, so that neither is visible without the other.
///
/// On the pool the transaction is a `BEGIN`; inside a caller's transaction
/// it becomes a savepoint, and the caller decides the fate of the whole.
pub async fn complete_thread<C: ConnectionTrait + TransactionTrait>(
    db: &C,
    id: i32,
) -> Result<Option<thread::Model>, DbErr> {
    let txn = db.begin().await?;
    let Some(thread) = Thread::find_by_id(id).one(&txn).await? else {
        return Ok(None);
    };
    let mut draft: thread::ActiveModel = thread.into();
    draft.status = Set(thread::Status::Completed);
    let completed = draft.update(&txn).await?;
    let seq = ThreadRecord::find()
        .filter(thread_record::Column::ThreadId.eq(id))
        .count(&txn)
        .await?;
    thread_record::ActiveModel {
        thread_id: Set(id),
        seq: Set(i32::try_from(seq).unwrap_or(i32::MAX).saturating_add(1)),
        role: Set(Role::System),
        content: Set(Some("Thread completed.".to_owned())),
        ..Default::default()
    }
    .insert(&txn)
    .await?;
    txn.commit().await?;
    Ok(Some(completed))
}
// --8<-- [end:begin_commit]

// --8<-- [start:savepoint]
/// Opens a thread with its tools in one transaction. Each tool is created
/// under a savepoint: when it already exists, the failed insert is undone
/// by `ROLLBACK TO` and the existing tool is used, while the thread and the
/// other tools stay in the outer transaction.
pub async fn open_thread_with_tools<C: ConnectionTrait + TransactionTrait>(
    db: &C,
    body: OpenWithTools,
) -> Result<ThreadWithTools, DbErr> {
    let txn = db.begin().await?;
    let thread = thread::ActiveModel {
        agent_id: Set(body.agent_id),
        title: Set(body.title),
        ..Default::default()
    }
    .insert(&txn)
    .await?;
    let mut tools = Vec::new();
    for name in body.tools {
        let sp = txn.begin().await?;
        let draft = tool::ActiveModel {
            name: Set(name.clone()),
            ..Default::default()
        };
        let tool = match draft.insert(&sp).await {
            Ok(created) => {
                sp.commit().await?;
                created
            }
            Err(err) if err.is_unique_violation() => {
                sp.rollback().await?;
                Tool::find()
                    .filter(tool::Column::Name.eq(name))
                    .one(&txn)
                    .await?
                    .ok_or_else(|| DbErr::RecordNotFound("tool".to_owned()))?
            }
            Err(err) => return Err(err),
        };
        thread_tool::ActiveModel {
            thread_id: Set(thread.id),
            tool_id: Set(tool.id),
        }
        .insert(&txn)
        .await?;
        tools.push(tool);
    }
    txn.commit().await?;
    Ok(ThreadWithTools { thread, tools })
}
// --8<-- [end:savepoint]

// --8<-- [start:self_reference]
/// Opens a thread forked from another: the self-reference is a plain column.
pub async fn fork_thread<C: ConnectionTrait>(
    db: &C,
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
pub async fn parent_of<C: ConnectionTrait>(
    db: &C,
    fork: &thread::Model,
) -> Result<Option<thread::Model>, DbErr> {
    fork.find_related(Thread).one(db).await
}

/// The threads forked from this one: the reverse side, through its definition.
pub async fn forks_of<C: ConnectionTrait>(
    db: &C,
    root: &thread::Model,
) -> Result<Vec<thread::Model>, DbErr> {
    Thread::find()
        .related_to(&thread::Relation::Children.def(), root)
        .all(db)
        .await
}

/// The threads an agent reviews: the second relation to `agent`, joined
/// under an alias so that the filter can name it.
pub async fn reviewed_by<C: ConnectionTrait>(
    db: &C,
    agent_name: &str,
) -> Result<Vec<thread::Model>, DbErr> {
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

// --8<-- [start:projections]
/// The open threads as a partial model that selects its own columns.
pub async fn summaries<C: ConnectionTrait>(db: &C) -> Result<Vec<ThreadSummary>, DbErr> {
    Thread::find()
        .filter(thread::Column::Status.eq(thread::Status::Open))
        .into_partial_model::<ThreadSummary>()
        .all(db)
        .await
}

/// Ids and titles as tuples, decoded by position.
pub async fn titles<C: ConnectionTrait>(db: &C) -> Result<Vec<(i32, String)>, DbErr> {
    Thread::find()
        .select_only()
        .column(thread::Column::Id)
        .column_as(thread::Column::Title, "title")
        .into_tuple::<(i32, String)>()
        .all(db)
        .await
}

/// The first thread as a JSON object keyed by column name.
pub async fn first_as_json<C: ConnectionTrait>(db: &C) -> Result<Option<serde_json::Value>, DbErr> {
    Thread::find().into_json().one(db).await
}

/// Whether at least one thread is completed, without reading any.
pub async fn any_completed<C: ConnectionTrait>(db: &C) -> Result<bool, DbErr> {
    Thread::find()
        .filter(thread::Column::Status.eq(thread::Status::Completed))
        .exists(db)
        .await
}
// --8<-- [end:projections]
