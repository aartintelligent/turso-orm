//! The tools a thread invoked: the junction between threads and tools, read
//! in both directions.

use crate::entity::{Thread, Tool};
use crate::entity::{thread, thread_tool, tool};
use turso_orm::ConnectionTrait;
use turso_orm::prelude::*;

use crate::dto::thread::ThreadWithTools;

/// Records that a thread invoked a tool, creating the tool on first sight.
pub async fn record_tool_use<C: ConnectionTrait>(
    db: &C,
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
pub async fn tools_of_thread<C: ConnectionTrait>(
    db: &C,
    thread: &thread::Model,
) -> Result<Vec<tool::Model>, DbErr> {
    thread.find_related(Tool).all(db).await
}

/// The threads that invoked a tool: a join through the junction lets the
/// filter mention the other side.
pub async fn threads_using<C: ConnectionTrait>(
    db: &C,
    tool_name: &str,
) -> Result<Vec<thread::Model>, DbErr> {
    Thread::find()
        .inner_join(Tool)
        .filter(tool::Column::Name.eq(tool_name))
        .all(db)
        .await
}

/// Every thread with all of its tools: one query, one entry per thread.
pub async fn threads_with_tools<C: ConnectionTrait>(db: &C) -> Result<Vec<ThreadWithTools>, DbErr> {
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
pub async fn tools_for<C: ConnectionTrait>(
    db: &C,
    threads: &[thread::Model],
) -> Result<Vec<Vec<tool::Model>>, DbErr> {
    threads.load_many(Tool, db).await
}
// --8<-- [end:many_to_many]
