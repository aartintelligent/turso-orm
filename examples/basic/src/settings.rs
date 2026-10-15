//! The database settings: defaults in code, overridden by `DATABASE__*` environment variables.
//!
//! `Settings::default()` is the only source of truth for the defaults; the
//! environment layers on top of it, so that `DATABASE__MODE=sync` with
//! `DATABASE__URL` and `DATABASE__AUTH_TOKEN` is all a deployment needs to
//! say. No file is read. Where the server listens and what tracing shows
//! are not settings: they are constants of `main.rs` and `telemetry.rs`.

use std::fmt;

use config::{Config, ConfigError, Environment};
use serde::{Deserialize, Serialize};

/// Where the database lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// In memory, for as long as the process runs.
    Memory,
    /// A local file at `path`.
    File,
    /// A local file kept in sync with a Turso Cloud database at `url`.
    Sync,
    /// Turso Cloud over HTTP at `url`, no local disk.
    Remote,
}

/// How to open the database.
#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    /// The mode.
    pub mode: Mode,
    /// The file, for the `file` and `sync` modes.
    pub path: String,
    /// The Turso Cloud URL, for the `sync` and `remote` modes.
    pub url: String,
    /// The Turso Cloud token, for the `sync` and `remote` modes.
    pub auth_token: String,
    /// Whether the engine enforces foreign keys.
    pub foreign_keys: bool,
    /// The size of the pool.
    pub max_connections: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: Mode::Memory,
            path: "basic.db".to_owned(),
            url: String::new(),
            auth_token: String::new(),
            foreign_keys: true,
            max_connections: 8,
        }
    }
}

/// Prints every setting but the token, which a log must never show.
impl fmt::Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Settings")
            .field("mode", &self.mode)
            .field("path", &self.path)
            .field("url", &self.url)
            .field("auth_token", &"<redacted>")
            .field("foreign_keys", &self.foreign_keys)
            .field("max_connections", &self.max_connections)
            .finish()
    }
}

impl Settings {
    /// The defaults, overridden by the `DATABASE__*` environment variables.
    pub fn load() -> Result<Self, ConfigError> {
        Config::builder()
            .add_source(Config::try_from(&Self::default())?)
            .add_source(
                Environment::with_prefix("DATABASE")
                    .separator("__")
                    .try_parsing(true),
            )
            .build()?
            .try_deserialize()
    }
}
