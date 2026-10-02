//! Bound parameter values, modeled by [`Value`].
//!
//! SQLite stores exactly five storage classes, so the value model is just
//! those five: there is no boolean, date or decimal variant to invent a
//! representation for. Richer Rust types are flattened at the edge through
//! `From` implementations — booleans become `0` / `1`, dates and times
//! become ISO 8601 text, UUIDs hyphenated text, JSON its textual form and
//! decimals their exact decimal text — so the writer only ever binds what
//! the engine natively understands. The reverse direction, decoding a column
//! into a Rust type, lives in `turso-orm-driver` where the requested type is
//! known.
//!
//! Values are always bound as parameters. [`Value::to_literal`] exists for
//! logs and tests and is never used to execute anything, which keeps SQL
//! injection out of the picture by construction.

use std::fmt;

/// A bound parameter or literal.
///
/// SQLite has exactly these storage classes. Richer Rust types are flattened
/// through [`From`] implementations: booleans become `0` / `1`, dates and
/// times become ISO 8601 text, UUIDs become hyphenated text, JSON becomes its
/// textual form and decimals become their exact decimal text.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// SQL `NULL`.
    Null,
    /// A 64-bit signed integer.
    Integer(i64),
    /// A 64-bit float.
    Real(f64),
    /// UTF-8 text.
    Text(String),
    /// Raw bytes.
    Blob(Vec<u8>),
}

impl Value {
    /// Whether this is `NULL`.
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Renders the value as a SQL literal.
    ///
    /// This is for debugging output only; execution always binds parameters,
    /// so the escaping here never has to be injection-proof against the
    /// engine, only readable.
    pub fn to_literal(&self) -> String {
        match self {
            Value::Null => "NULL".to_owned(),
            Value::Integer(n) => n.to_string(),
            Value::Real(f) => {
                // An integral float is printed with one decimal so that it
                // stays recognisable as a REAL rather than an INTEGER.
                if f.fract() == 0.0 && f.is_finite() {
                    format!("{f:.1}")
                } else {
                    f.to_string()
                }
            }
            Value::Text(s) => format!("'{}'", s.replace('\'', "''")),
            Value::Blob(b) => {
                use std::fmt::Write as _;
                let mut out = String::with_capacity(b.len() * 2 + 3);
                out.push_str("X'");
                for byte in b {
                    let _ = write!(out, "{byte:02X}");
                }
                out.push('\'');
                out
            }
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_literal())
    }
}

/// Implements `From<$t> for Value` for integer types that widen losslessly
/// into `i64`.
macro_rules! int_from {
    ($($t:ty),*) => {$(
        impl From<$t> for Value {
            fn from(v: $t) -> Self {
                Value::Integer(i64::from(v))
            }
        }
    )*};
}
int_from!(i8, i16, i32, i64, u8, u16, u32);

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Integer(i64::from(v))
    }
}

impl From<f32> for Value {
    fn from(v: f32) -> Self {
        Value::Real(f64::from(v))
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Real(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Text(v.to_owned())
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Text(v)
    }
}

impl From<&String> for Value {
    fn from(v: &String) -> Self {
        Value::Text(v.clone())
    }
}

impl From<char> for Value {
    fn from(v: char) -> Self {
        Value::Text(v.to_string())
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Value::Blob(v)
    }
}

impl From<&[u8]> for Value {
    fn from(v: &[u8]) -> Self {
        Value::Blob(v.to_vec())
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Self {
        v.map_or(Value::Null, Into::into)
    }
}

impl<T: Into<Value> + Clone> From<&T> for Value
where
    T: ValueRefInto,
{
    fn from(v: &T) -> Self {
        v.clone().into()
    }
}

/// Marker that lets `&T` convert into a [`Value`] for owned-value types.
///
/// A blanket `From<&T>` would conflict with the dedicated `&str`, `&[u8]`
/// and `&String` implementations, so the types that convert by cloning opt in
/// through this marker instead.
pub trait ValueRefInto {}
impl ValueRefInto for bool {}
impl ValueRefInto for i8 {}
impl ValueRefInto for i16 {}
impl ValueRefInto for i32 {}
impl ValueRefInto for i64 {}
impl ValueRefInto for u8 {}
impl ValueRefInto for u16 {}
impl ValueRefInto for u32 {}
impl ValueRefInto for f32 {}
impl ValueRefInto for f64 {}
impl ValueRefInto for Vec<u8> {}
impl<T: ValueRefInto> ValueRefInto for Option<T> {}

/// Conversions for the `chrono` date and time types.
///
/// Naive values are written in the `YYYY-MM-DD HH:MM:SS.fff` family that
/// SQLite's date functions understand; zoned values are written as RFC 3339
/// so the offset survives the round trip.
#[cfg(feature = "with-chrono")]
#[cfg_attr(docsrs, doc(cfg(feature = "with-chrono")))]
mod chrono_impls {
    use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};

    use super::Value;

    /// The text format used for naive timestamps.
    ///
    /// The fractional seconds are emitted only when non-zero, which keeps
    /// whole-second timestamps identical to what SQLite's `datetime()`
    /// produces.
    pub const NAIVE_DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S%.f";

    impl From<NaiveDate> for Value {
        fn from(v: NaiveDate) -> Self {
            Value::Text(v.format("%Y-%m-%d").to_string())
        }
    }
    impl From<NaiveTime> for Value {
        fn from(v: NaiveTime) -> Self {
            Value::Text(v.format("%H:%M:%S%.f").to_string())
        }
    }
    impl From<NaiveDateTime> for Value {
        fn from(v: NaiveDateTime) -> Self {
            Value::Text(v.format(NAIVE_DATETIME_FORMAT).to_string())
        }
    }
    impl<Tz: TimeZone> From<DateTime<Tz>> for Value {
        fn from(v: DateTime<Tz>) -> Self {
            Value::Text(v.to_rfc3339())
        }
    }
    impl super::ValueRefInto for NaiveDate {}
    impl super::ValueRefInto for NaiveTime {}
    impl super::ValueRefInto for NaiveDateTime {}
    impl super::ValueRefInto for DateTime<Utc> {}
    impl super::ValueRefInto for DateTime<FixedOffset> {}
}
#[cfg(feature = "with-chrono")]
#[cfg_attr(docsrs, doc(cfg(feature = "with-chrono")))]
pub use chrono_impls::NAIVE_DATETIME_FORMAT;

#[cfg(feature = "with-uuid")]
#[cfg_attr(docsrs, doc(cfg(feature = "with-uuid")))]
impl From<uuid::Uuid> for Value {
    fn from(v: uuid::Uuid) -> Self {
        Value::Text(v.hyphenated().to_string())
    }
}
#[cfg(feature = "with-uuid")]
impl ValueRefInto for uuid::Uuid {}

#[cfg(feature = "with-json")]
#[cfg_attr(docsrs, doc(cfg(feature = "with-json")))]
impl From<serde_json::Value> for Value {
    fn from(v: serde_json::Value) -> Self {
        Value::Text(v.to_string())
    }
}
#[cfg(feature = "with-json")]
impl ValueRefInto for serde_json::Value {}

#[cfg(feature = "with-rust_decimal")]
#[cfg_attr(docsrs, doc(cfg(feature = "with-rust_decimal")))]
impl From<rust_decimal::Decimal> for Value {
    fn from(v: rust_decimal::Decimal) -> Self {
        Value::Text(v.to_string())
    }
}
#[cfg(feature = "with-rust_decimal")]
impl ValueRefInto for rust_decimal::Decimal {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Literal rendering flattens booleans, escapes quotes, hex-encodes
    /// blobs, maps `None` to `NULL` and keeps integral floats recognisable.
    #[test]
    fn literals() {
        assert_eq!(Value::from(true).to_literal(), "1");
        assert_eq!(Value::from("it's").to_literal(), "'it''s'");
        assert_eq!(Value::from(vec![0xAB, 0x01]).to_literal(), "X'AB01'");
        assert_eq!(Value::from(Option::<i32>::None), Value::Null);
        assert_eq!(Value::from(2.0f64).to_literal(), "2.0");
    }
}
