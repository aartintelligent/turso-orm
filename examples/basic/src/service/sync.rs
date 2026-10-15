//! Keeping an embedded replica in step with Turso Cloud.
//!
//! A replica writes to its local file only: nothing leaves the machine until
//! `push`, and nothing arrives until `pull`. The driver exposes both on the
//! pool; this module is the one place the example calls them.

use turso_orm::prelude::*;

// --8<-- [start:push_pull]
/// Sends the replica's local writes to Turso Cloud, then brings back what
/// other writers committed there; `true` when the pull changed the replica.
///
/// Both calls fail with `DbErr::Driver` on a database that is not a replica,
/// which is why the handler checks the mode before calling this.
pub async fn push_pull(db: &Database) -> Result<bool, DbErr> {
    db.push().await?;
    Ok(db.pull().await?)
}
// --8<-- [end:push_pull]
