//! The options a database is opened with, modeled by [`ConnectOptions`].
//!
//! The options cover three layers that the rest of the crate keeps apart:
//! how the engine opens the file (source, read-only, encryption, the
//! experimental feature flags of `turso::Builder`), how the pool behaves
//! (size and acquire timeout) and what every new connection is configured
//! with (busy timeout, `foreign_keys`, MVCC and arbitrary pragmas). This
//! module owns the data and the translation into a `turso::Builder`; it
//! does not open anything, which is [`Database::connect`]'s job.
//!
//! Defaults follow what an application usually wants rather than SQLite's
//! own: foreign keys are enforced, a lock is waited on for five seconds and
//! the pool holds eight connections.
//!
//! - [`ConnectOptions`]: the builder;
//! - [`Source`], [`Encryption`], [`Experimental`]: the pieces of it;
//! - `SyncOptions`: the embedded-replica settings, behind the `sync`
//!   feature;
//! - `RemoteOptions`: the Turso Cloud HTTP settings, behind the
//!   `serverless` feature.
//!
//! [`Database::connect`]: crate::Database::connect

use std::path::{Path, PathBuf};
use std::time::Duration;

/// Where the database lives.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Source {
    /// A private in-memory database shared by every connection of the pool.
    Memory,
    /// A local database file, created if missing.
    File(PathBuf),
    /// A local file kept in sync with a Turso Cloud database — an embedded
    /// replica.
    #[cfg(feature = "sync")]
    #[cfg_attr(docsrs, doc(cfg(feature = "sync")))]
    Sync(SyncOptions),
    /// A Turso Cloud database reached over HTTP, with no local file.
    #[cfg(feature = "serverless")]
    #[cfg_attr(docsrs, doc(cfg(feature = "serverless")))]
    Remote(RemoteOptions),
}

/// The settings of a remote Turso Cloud database.
#[cfg(feature = "serverless")]
#[cfg_attr(docsrs, doc(cfg(feature = "serverless")))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct RemoteOptions {
    /// The database URL (`libsql://`, `turso://` or `https://`).
    pub url: String,
    /// The bearer token for the database.
    pub auth_token: Option<String>,
    /// The base64 key of a database encrypted with a customer-managed key.
    pub remote_encryption_key: Option<String>,
}

/// The settings of an embedded replica synchronised with Turso Cloud.
#[cfg(feature = "sync")]
#[cfg_attr(docsrs, doc(cfg(feature = "sync")))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncOptions {
    /// The local replica file.
    pub path: PathBuf,
    /// The remote database URL (`libsql://`, `turso://` or `https://`).
    pub remote_url: String,
    /// The bearer token for the remote database.
    pub auth_token: Option<String>,
    /// Whether to download the remote database on first open when the local
    /// file is empty.
    pub bootstrap_if_empty: bool,
}

/// The encryption-at-rest settings — experimental in Turso.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Encryption {
    /// The cipher name, for example `aes256gcm`.
    pub cipher: String,
    /// The hex-encoded key.
    pub hexkey: String,
}

/// Turso engine features that are opt-in because they are still
/// experimental.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the flags mirror the independent flags of turso::Builder"
)]
pub struct Experimental {
    /// `ATTACH` and `DETACH`.
    pub attach: bool,
    /// `CREATE TYPE`, `CREATE DOMAIN` and array column types.
    pub custom_types: bool,
    /// Virtual generated columns.
    pub generated_columns: bool,
    /// `CREATE INDEX ... USING fts` and vector indexes.
    pub index_method: bool,
    /// `CREATE MATERIALIZED VIEW`.
    pub materialized_views: bool,
    /// In-place `VACUUM`.
    pub vacuum: bool,
    /// Several OS processes opening the same file.
    pub multiprocess_wal: bool,
    /// `WITHOUT ROWID` tables.
    pub without_rowid: bool,
}

/// The options for opening a Turso database.
///
/// ```
/// use std::time::Duration;
/// use turso_orm_driver::ConnectOptions;
///
/// let opts = ConnectOptions::new("app.db")
///     .max_connections(4)
///     .busy_timeout(Duration::from_secs(5))
///     .foreign_keys(true);
/// assert_eq!(opts.max_connections_value(), 4);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectOptions {
    /// Where the database lives.
    pub(crate) source: Source,
    /// Whether to open read-only.
    pub(crate) read_only: bool,
    /// The encryption-at-rest settings, if any.
    pub(crate) encryption: Option<Encryption>,
    /// The experimental engine features to enable.
    pub(crate) experimental: Experimental,
    /// The pool size, at least one.
    pub(crate) max_connections: usize,
    /// How long to wait for a free pooled connection.
    pub(crate) acquire_timeout: Duration,
    /// How long a connection waits on a lock; `None` fails immediately.
    pub(crate) busy_timeout: Option<Duration>,
    /// Whether to enforce foreign keys on every connection.
    pub(crate) foreign_keys: bool,
    /// Whether to switch the journal mode to MVCC on every connection.
    pub(crate) mvcc: bool,
    /// Extra `PRAGMA name = value` pairs run on every new connection.
    pub(crate) pragmas: Vec<(String, String)>,
}

impl ConnectOptions {
    /// The defaults for any source.
    fn with_source(source: Source) -> Self {
        Self {
            source,
            read_only: false,
            encryption: None,
            experimental: Experimental::default(),
            max_connections: 8,
            acquire_timeout: Duration::from_secs(30),
            busy_timeout: Some(Duration::from_secs(5)),
            foreign_keys: true,
            mvcc: false,
            pragmas: Vec::new(),
        }
    }

    /// Opens, or creates, the database file at `path`; `":memory:"` opens
    /// an in-memory database.
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        if path.as_os_str() == ":memory:" {
            Self::in_memory()
        } else {
            Self::with_source(Source::File(path.to_path_buf()))
        }
    }

    /// Opens a private in-memory database.
    pub fn in_memory() -> Self {
        Self::with_source(Source::Memory)
    }

    /// Opens a local replica of a Turso Cloud database.
    #[cfg(feature = "sync")]
    #[cfg_attr(docsrs, doc(cfg(feature = "sync")))]
    pub fn sync(path: impl AsRef<Path>, remote_url: impl Into<String>) -> Self {
        Self::with_source(Source::Sync(SyncOptions {
            path: path.as_ref().to_path_buf(),
            remote_url: remote_url.into(),
            auth_token: None,
            bootstrap_if_empty: true,
        }))
    }

    /// Opens a Turso Cloud database over HTTP, without a local file.
    ///
    /// Every statement is an HTTP request, so the busy timeout and the
    /// MVCC setting do not apply; the pool, the pragmas and the
    /// transactions do, since each connection is a server-side session.
    #[cfg(feature = "serverless")]
    #[cfg_attr(docsrs, doc(cfg(feature = "serverless")))]
    pub fn remote(url: impl Into<String>) -> Self {
        Self::with_source(Source::Remote(RemoteOptions {
            url: url.into(),
            auth_token: None,
            remote_encryption_key: None,
        }))
    }

    /// Sets the bearer token of a remote database or of an embedded
    /// replica's remote; ignored for local sources.
    #[cfg(any(feature = "sync", feature = "serverless"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "sync", feature = "serverless"))))]
    #[must_use]
    pub fn auth_token(mut self, token: impl Into<String>) -> Self {
        match &mut self.source {
            #[cfg(feature = "sync")]
            Source::Sync(opts) => opts.auth_token = Some(token.into()),
            #[cfg(feature = "serverless")]
            Source::Remote(opts) => opts.auth_token = Some(token.into()),
            _ => {}
        }
        self
    }

    /// Sets the base64 key of a remote database encrypted with a
    /// customer-managed key; ignored for other sources.
    #[cfg(feature = "serverless")]
    #[cfg_attr(docsrs, doc(cfg(feature = "serverless")))]
    #[must_use]
    pub fn remote_encryption_key(mut self, key: impl Into<String>) -> Self {
        if let Source::Remote(opts) = &mut self.source {
            opts.remote_encryption_key = Some(key.into());
        }
        self
    }

    /// Opens the database in read-only mode.
    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Enables encryption at rest — experimental in Turso.
    #[must_use]
    pub fn encryption(mut self, cipher: impl Into<String>, hexkey: impl Into<String>) -> Self {
        self.encryption = Some(Encryption {
            cipher: cipher.into(),
            hexkey: hexkey.into(),
        });
        self
    }

    /// Enables experimental engine features.
    #[must_use]
    pub fn experimental(mut self, experimental: Experimental) -> Self {
        self.experimental = experimental;
        self
    }

    /// Sets the maximum number of pooled connections (minimum 1, default 8).
    #[must_use]
    pub fn max_connections(mut self, max: usize) -> Self {
        self.max_connections = max.max(1);
        self
    }

    /// Sets how long to wait for a free pooled connection (default 30 s).
    #[must_use]
    pub fn acquire_timeout(mut self, timeout: Duration) -> Self {
        self.acquire_timeout = timeout;
        self
    }

    /// Sets how long a connection waits on a lock before failing with a busy
    /// error (default 5 s); `None` fails immediately.
    #[must_use]
    pub fn busy_timeout(mut self, timeout: impl Into<Option<Duration>>) -> Self {
        self.busy_timeout = timeout.into();
        self
    }

    /// Enforces foreign keys (`PRAGMA foreign_keys = ON`, default on).
    #[must_use]
    pub fn foreign_keys(mut self, enabled: bool) -> Self {
        self.foreign_keys = enabled;
        self
    }

    /// Switches the database to MVCC (`PRAGMA journal_mode = 'mvcc'`), which
    /// enables `BEGIN CONCURRENT` and multiple concurrent writers.
    ///
    /// Experimental in Turso; conflicts surface at commit as busy errors.
    #[must_use]
    pub fn mvcc(mut self, enabled: bool) -> Self {
        self.mvcc = enabled;
        self
    }

    /// Runs `PRAGMA name = value` on every new connection.
    #[must_use]
    pub fn pragma(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.pragmas.push((name.into(), value.into()));
        self
    }

    /// The configured source.
    pub fn source(&self) -> &Source {
        &self.source
    }

    /// The configured pool size.
    pub fn max_connections_value(&self) -> usize {
        self.max_connections
    }

    /// The configured busy timeout.
    pub fn busy_timeout_value(&self) -> Option<Duration> {
        self.busy_timeout
    }

    /// Whether this points at the in-memory database.
    pub fn is_in_memory(&self) -> bool {
        matches!(self.source, Source::Memory)
    }

    /// Builds the engine builder for a local database at `path`, applying
    /// the read-only, encryption and experimental settings.
    pub(crate) fn local_builder(&self, path: &str) -> turso::Builder {
        let mut builder = turso::Builder::new_local(path).read_only(self.read_only);
        if let Some(enc) = &self.encryption {
            builder =
                builder
                    .experimental_encryption(true)
                    .with_encryption(turso::EncryptionOpts {
                        cipher: enc.cipher.clone(),
                        hexkey: enc.hexkey.clone(),
                    });
        }
        let x = self.experimental;
        builder
            .experimental_attach(x.attach)
            .experimental_custom_types(x.custom_types)
            .experimental_generated_columns(x.generated_columns)
            .experimental_index_method(x.index_method)
            .experimental_materialized_views(x.materialized_views)
            .experimental_vacuum(x.vacuum)
            .experimental_multiprocess_wal(x.multiprocess_wal)
            .experimental_without_rowid(x.without_rowid)
    }
}

impl From<&str> for ConnectOptions {
    fn from(path: &str) -> Self {
        Self::new(path)
    }
}

impl From<String> for ConnectOptions {
    fn from(path: String) -> Self {
        Self::new(path)
    }
}

impl From<PathBuf> for ConnectOptions {
    fn from(path: PathBuf) -> Self {
        Self::new(path)
    }
}

impl From<&Path> for ConnectOptions {
    fn from(path: &Path) -> Self {
        Self::new(path)
    }
}
