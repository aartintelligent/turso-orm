//! The settings: defaults in code, overridden by `BASIC__*` environment variables.
//!
//! `Settings::default()` is the only source of truth for the defaults; the
//! environment layers on top of it, with `__` between levels, so that
//! `BASIC__DATABASE__MODE=sync` or `BASIC__SERVER__ADDR=0.0.0.0:8080` is
//! all a deployment needs to say. No file is read.

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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatabaseSettings {
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

/// Where the server listens.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerSettings {
    /// The address to bind.
    pub addr: String,
}

/// What tracing shows.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogSettings {
    /// An `EnvFilter` directive, such as `info` or `turso_orm_driver=debug`.
    pub filter: String,
}

/// Every setting of the application.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub server: ServerSettings,
    pub log: LogSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            database: DatabaseSettings {
                mode: Mode::Memory,
                path: "basic.db".to_owned(),
                url: String::new(),
                auth_token: String::new(),
                foreign_keys: true,
                max_connections: 8,
            },
            server: ServerSettings {
                addr: "127.0.0.1:3000".to_owned(),
            },
            log: LogSettings {
                filter: "info,turso_orm_driver=debug".to_owned(),
            },
        }
    }
}

impl Settings {
    /// The defaults, overridden by the `BASIC__*` environment variables.
    pub fn load() -> Result<Self, ConfigError> {
        Config::builder()
            .add_source(Config::try_from(&Self::default())?)
            .add_source(
                Environment::with_prefix("BASIC")
                    .separator("__")
                    .try_parsing(true),
            )
            .build()?
            .try_deserialize()
    }
}
