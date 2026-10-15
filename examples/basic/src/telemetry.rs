//! Sets up tracing.

use tracing_subscriber::EnvFilter;

/// What tracing shows: the application at `info`, and every statement the
/// driver runs at `debug`. Not a setting: change it here.
const FILTER: &str = "info,turso_orm_driver=debug";

/// Installs the global subscriber.
pub fn init() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(FILTER))
        .init();
}
