//! Opens the database in the mode the settings name.

use turso_orm::prelude::*;

use crate::settings::{DatabaseSettings, Mode};

// --8<-- [start:connect]
/// Opens the database the settings describe: four modes, one `ConnectOptions` each.
///
/// The entities, the queries, the transactions and the migrations are the
/// same in every mode; only this function knows where the data lives.
pub async fn connect(settings: &DatabaseSettings) -> Result<Database, DbErr> {
    let options = match settings.mode {
        Mode::Memory => ConnectOptions::in_memory(),
        Mode::File => ConnectOptions::new(&settings.path),
        Mode::Sync => ConnectOptions::sync(&settings.path, settings.url.clone())
            .auth_token(settings.auth_token.clone()),
        Mode::Remote => {
            ConnectOptions::remote(settings.url.clone()).auth_token(settings.auth_token.clone())
        }
    };
    let db = Database::connect(
        options
            .foreign_keys(settings.foreign_keys)
            .max_connections(settings.max_connections),
    )
    .await?;
    Ok(db)
}
// --8<-- [end:connect]
