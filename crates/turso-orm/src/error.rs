//! The ORM error type, modeled by [`DbErr`].
//!
//! Every fallible operation in the crate returns [`DbErr`]. The driver's own
//! error is wrapped transparently in [`DbErr::Driver`] so that callers keep
//! access to its classification (busy, constraint kind, decode failure)
//! through the helpers on [`DbErr`]; the remaining variants cover the
//! situations the ORM layer detects itself, such as a `NotSet` primary key or
//! an `UPDATE` that matched nothing.
//!
//! The enum is `#[non_exhaustive]` so that new ORM-level failure modes can be
//! added without breaking matches downstream.

use turso_orm_driver::{ConstraintKind, Error as DriverError, ErrorKind};

/// The error returned by every fallible operation of the ORM.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DbErr {
    /// An error reported by the driver or the engine.
    #[error(transparent)]
    Driver(#[from] DriverError),
    /// A row was expected but none was found.
    #[error("record not found: {0}")]
    RecordNotFound(String),
    /// An `INSERT` inserted nothing, for example under `ON CONFLICT DO NOTHING`.
    #[error("record not inserted")]
    RecordNotInserted,
    /// An `UPDATE` matched no row.
    #[error("record not updated")]
    RecordNotUpdated,
    /// A required attribute of an active model is `NotSet`.
    #[error("attribute not set: {0}")]
    AttrNotSet(String),
    /// A primary key value is missing from an active model.
    #[error("primary key not set")]
    PrimaryKeyNotSet,
    /// A value had the wrong type for the target column or field.
    #[error("type error: {0}")]
    Type(String),
    /// A JSON conversion failed.
    #[error("json error: {0}")]
    Json(String),
    /// A migration failed.
    #[error("migration error: {0}")]
    Migration(String),
    /// A free-form error raised by user code or hooks.
    #[error("{0}")]
    Custom(String),
}

impl DbErr {
    /// The classification of the underlying driver error, if any.
    ///
    /// ORM-level variants carry no driver classification and yield `None`.
    pub fn kind(&self) -> Option<ErrorKind> {
        match self {
            DbErr::Driver(e) => Some(e.kind()),
            _ => None,
        }
    }

    /// The violated constraint, when this wraps a constraint error.
    pub fn constraint(&self) -> Option<ConstraintKind> {
        match self {
            DbErr::Driver(e) => e.constraint(),
            _ => None,
        }
    }

    /// Whether the error is transient lock contention worth retrying.
    pub fn is_busy(&self) -> bool {
        matches!(self, DbErr::Driver(e) if e.is_busy())
    }

    /// Whether the error is a unique-key violation.
    pub fn is_unique_violation(&self) -> bool {
        self.constraint() == Some(ConstraintKind::Unique)
    }

    /// Whether the error is a foreign-key violation.
    pub fn is_foreign_key_violation(&self) -> bool {
        self.constraint() == Some(ConstraintKind::ForeignKey)
    }
}

/// The result alias used throughout the crate, defaulting to [`DbErr`].
pub type Result<T, E = DbErr> = std::result::Result<T, E>;
