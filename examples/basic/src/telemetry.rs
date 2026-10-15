//! Sets up tracing.

use tracing_subscriber::EnvFilter;

/// What tracing shows: the application at `info`, every statement, pragma
/// and transaction boundary of the driver at `debug`, and the engine's own
/// synchronisation log at `warn` only, since it narrates every step of a
/// pull at `info`. Not a setting: change it here.
const FILTER: &str = "info,turso_orm_driver=debug,turso_sync_engine=warn";

/// Installs the global subscriber.
pub fn init() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(FILTER))
        .init();
}
