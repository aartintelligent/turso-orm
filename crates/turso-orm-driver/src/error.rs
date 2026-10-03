//! The driver's error type and its classification, modeled by [`Error`].
//!
//! Callers rarely need the exact engine error; they need to know whether to
//! retry, whether a constraint was hit and which one, or whether the
//! connection is gone. [`ErrorKind`] answers those questions uniformly for
//! engine errors and for the driver's own failures, so retry loops and
//! upsert fallbacks can be written once.
//!
//! The classification is partly textual because `turso::Error` carries only
//! a string for most variants: the variant gives the broad kind, and for
//! constraint violations and MVCC write conflicts the message is inspected.
//! This module owns that mapping; it does not retry or log anything itself.
//!
//! - [`Error`]: the enum returned by every fallible operation of the crate;
//! - [`ErrorKind`] and [`ConstraintKind`]: the classification;
//! - [`Result`]: the crate's result alias.

use std::fmt;

/// The crate's result alias.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Which constraint a statement violated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConstraintKind {
    /// A `UNIQUE` or `PRIMARY KEY` constraint.
    Unique,
    /// A `FOREIGN KEY` constraint.
    ForeignKey,
    /// A `NOT NULL` constraint.
    NotNull,
    /// A `CHECK` constraint.
    Check,
    /// Any other constraint.
    Other,
}

/// The classification of an [`Error`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// Lock contention, a stale snapshot or an MVCC commit conflict. Worth
    /// retrying.
    Busy,
    /// A constraint violation.
    Constraint(ConstraintKind),
    /// The database could not be opened or a connection could not be
    /// established.
    Connection,
    /// The pool timed out handing out a connection.
    PoolTimeout,
    /// A value could not be decoded into the requested Rust type.
    Decode,
    /// A value could not be encoded as a parameter.
    Encode,
    /// A query returned no row where one was required.
    NotFound,
    /// Misuse of the API, for example contradictory options or a closed
    /// pool.
    Misuse,
    /// Any other engine error.
    Other,
}

/// The crate's error type.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// An error reported by the Turso engine, already classified.
    #[error("{kind:?}: {source}")]
    Turso {
        /// The classification.
        kind: ErrorKind,
        /// The engine error.
        #[source]
        source: turso::Error,
    },
    /// An error reported by the serverless client, already classified.
    #[cfg(feature = "serverless")]
    #[cfg_attr(docsrs, doc(cfg(feature = "serverless")))]
    #[error("{kind:?}: {source}")]
    Remote {
        /// The classification.
        kind: ErrorKind,
        /// The client error.
        #[source]
        source: turso_serverless::Error,
    },
    /// No pooled connection became free within the acquire timeout.
    #[error("timed out waiting for a pooled connection")]
    PoolTimeout,
    /// A column could not be decoded into the requested Rust type.
    #[error("cannot decode column {column} as {ty}: {reason}")]
    Decode {
        /// The column name or index, as the caller asked for it.
        column: String,
        /// The requested Rust type.
        ty: &'static str,
        /// Why the conversion failed.
        reason: String,
    },
    /// A value could not be encoded as a parameter.
    #[error("cannot encode value: {0}")]
    Encode(String),
    /// A query returned no row where one was required.
    #[error("no row returned")]
    NotFound,
    /// The connect options are contradictory or unsupported.
    #[error("invalid connect options: {0}")]
    InvalidOptions(String),
    /// The API was used in a way it cannot honour.
    #[error("misuse: {0}")]
    Misuse(String),
    /// A free-form error, for callers layering on top of this crate.
    #[error("{0}")]
    Custom(String),
}

impl Error {
    /// Classifies the error.
    pub fn kind(&self) -> ErrorKind {
        match self {
            Error::Turso { kind, .. } => *kind,
            #[cfg(feature = "serverless")]
            Error::Remote { kind, .. } => *kind,
            Error::PoolTimeout => ErrorKind::PoolTimeout,
            Error::Decode { .. } => ErrorKind::Decode,
            Error::Encode(_) => ErrorKind::Encode,
            Error::NotFound => ErrorKind::NotFound,
            Error::InvalidOptions(_) | Error::Misuse(_) => ErrorKind::Misuse,
            Error::Custom(_) => ErrorKind::Other,
        }
    }

    /// Whether the error is transient lock contention worth retrying.
    pub fn is_busy(&self) -> bool {
        self.kind() == ErrorKind::Busy
    }

    /// The constraint kind, when this is a constraint violation.
    pub fn constraint(&self) -> Option<ConstraintKind> {
        match self.kind() {
            ErrorKind::Constraint(k) => Some(k),
            _ => None,
        }
    }

    /// Builds a decoding error for `column` and the requested type.
    ///
    /// Public so that hand-written and derived [`FromValue`](crate::FromValue)
    /// impls report failures in the same shape as the built-in decoders.
    pub fn decode(column: impl fmt::Display, ty: &'static str, reason: impl fmt::Display) -> Self {
        Error::Decode {
            column: column.to_string(),
            ty,
            reason: reason.to_string(),
        }
    }
}

impl From<turso::Error> for Error {
    fn from(source: turso::Error) -> Self {
        let kind = classify(&source);
        Error::Turso { kind, source }
    }
}

/// Classifies an engine error by variant, falling back to its message.
///
/// MVCC conflicts are reported as a generic `Error`, so that case is
/// matched textually by [`is_conflict`] to make it retryable like any
/// other busy condition.
fn classify(err: &turso::Error) -> ErrorKind {
    match err {
        turso::Error::Busy(_) | turso::Error::BusySnapshot(_) => ErrorKind::Busy,
        turso::Error::Constraint(msg) => ErrorKind::Constraint(classify_constraint(msg)),
        turso::Error::Misuse(_) => ErrorKind::Misuse,
        turso::Error::Error(msg) if is_conflict(msg) => ErrorKind::Busy,
        turso::Error::IoError(..) | turso::Error::NotAdb(_) | turso::Error::Corrupt(_) => {
            ErrorKind::Connection
        }
        _ => ErrorKind::Other,
    }
}

/// Classifies a serverless client error by variant, falling back to its
/// message the same way as for the engine. HTTP failures count as
/// connection errors: the statement never reached the database.
#[cfg(feature = "serverless")]
impl From<turso_serverless::Error> for Error {
    fn from(source: turso_serverless::Error) -> Self {
        use turso_serverless::Error as E;
        let kind = match &source {
            E::Busy(_) | E::BusySnapshot(_) => ErrorKind::Busy,
            E::Constraint(msg) => ErrorKind::Constraint(classify_constraint(msg)),
            E::Misuse(_) => ErrorKind::Misuse,
            E::Error(msg) if is_conflict(msg) => ErrorKind::Busy,
            E::Http(_) | E::NotAdb(_) | E::Corrupt(_) => ErrorKind::Connection,
            _ => ErrorKind::Other,
        };
        Error::Remote { kind, source }
    }
}

/// Whether a generic engine error reports an MVCC conflict, which a retry
/// of the whole transaction can resolve.
///
/// The engine words them `Write-write conflict`, `Conflict: …` and
/// `Database schema conflict`. Matching those phrases rather than the bare
/// word keeps deterministic errors that merely mention a conflict, such as
/// a parse error about `ON CONFLICT` clauses, out of the retryable kind.
fn is_conflict(msg: &str) -> bool {
    let lower = msg.to_ascii_lowercase();
    lower.contains("write-write conflict")
        || lower.starts_with("conflict:")
        || lower.contains("schema conflict")
}

/// Classifies a constraint violation from the engine's message, which is
/// the only place the constraint kind is reported.
fn classify_constraint(msg: &str) -> ConstraintKind {
    let lower = msg.to_ascii_lowercase();
    if lower.contains("unique") || lower.contains("primary key") {
        ConstraintKind::Unique
    } else if lower.contains("foreign key") {
        ConstraintKind::ForeignKey
    } else if lower.contains("not null") {
        ConstraintKind::NotNull
    } else if lower.contains("check") {
        ConstraintKind::Check
    } else {
        ConstraintKind::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Engine errors are classified by variant, and MVCC conflicts hidden
    /// in a generic error message are recognised as busy.
    #[test]
    fn classifies() {
        let e: Error = turso::Error::Constraint("UNIQUE constraint failed: t.a".into()).into();
        assert_eq!(e.constraint(), Some(ConstraintKind::Unique));
        let e: Error = turso::Error::Busy("database is locked".into()).into();
        assert!(e.is_busy());
        let e: Error = turso::Error::Error("write-write conflict".into()).into();
        assert!(e.is_busy());
        let e: Error = turso::Error::Error("syntax error".into()).into();
        assert_eq!(e.kind(), ErrorKind::Other);
    }

    /// Each conflict message of the engine is busy, while a parse error that
    /// mentions `ON CONFLICT` is not.
    #[test]
    fn classifies_conflicts_by_phrase() {
        for msg in [
            "Write-write conflict",
            "Conflict: row 3 was modified",
            "Database schema conflict",
        ] {
            let e: Error = turso::Error::Error(msg.into()).into();
            assert!(e.is_busy(), "{msg}");
        }
        let e: Error =
            turso::Error::Error("Parse error: conflicting ON CONFLICT clauses specified".into())
                .into();
        assert_eq!(e.kind(), ErrorKind::Other);
    }
}
