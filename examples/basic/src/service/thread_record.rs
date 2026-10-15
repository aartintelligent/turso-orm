//! The records of a thread: appended in order, in one transaction.

use crate::entity::{ThreadRecord, thread_record};
use turso_orm::TransactionTrait;
use turso_orm::prelude::*;

use crate::dto::thread_record::AppendRecord;

// --8<-- [start:transaction]
/// Appends records to a thread, numbered after the last one, in one transaction:
/// the sequence read and the insert either both happen or neither does.
pub async fn append_records<C: TransactionTrait>(
    db: &C,
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
