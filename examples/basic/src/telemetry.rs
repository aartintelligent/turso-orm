//! Sets up tracing from the log settings.

use tracing_subscriber::EnvFilter;

use crate::settings::LogSettings;

/// Installs the global subscriber with the filter of the settings.
///
/// A directive that does not parse falls back to `info`, so a typo in the
/// environment never keeps the server from starting.
pub fn init(settings: &LogSettings) {
    let filter = EnvFilter::try_new(&settings.filter).unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
