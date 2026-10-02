//! Decoding of storage values into Rust types, modeled by [`FromValue`].
//!
//! SQLite columns have affinity, not a type: an `INTEGER` column may hold
//! text, a `TEXT` column may hold a number, and a boolean is whatever the
//! application wrote. Decoding is therefore driven by the Rust type the
//! caller asks for rather than by the column's declared type, and it is
//! lenient in the way SQLite users expect — integers decode into booleans,
//! integral reals into integers, numbers into strings, text into dates,
//! UUIDs and JSON. The alternative, failing on any storage-class mismatch,
//! would make rows written by other tools unreadable.
//!
//! `Option<T>` is the only type that accepts `NULL`; every other type
//! reports a decoding error so that a nullable column read as a plain type
//! fails loudly instead of defaulting. Every error names the column and the
//! requested type.
//!
//! Values arrive as `turso_sql::Value`, the five storage classes, whichever
//! engine produced them; the executor converts from the engine's own value
//! type before a row is built.
//!
//! This module owns the conversions only. Rows and column lookup live in
//! `crate::executor`; the inverse direction, Rust values into parameters,
//! lives in `turso-sql`.

use turso_sql::Value;

use crate::error::{Error, Result};

/// Types that can be read out of a result column.
///
/// Decoding is lenient in the way SQLite users expect: integers decode into
/// booleans (`0` / `1`), reals accept integers, text parses into dates, UUIDs
/// and JSON, and blobs or text decode into UUIDs. `Option<T>` maps `NULL` to
/// `None`; any other type rejects `NULL`.
pub trait FromValue: Sized {
    /// The name used in error messages.
    const TYPE_NAME: &'static str;

    /// Decodes a value; `column` is only used for error messages.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Decode`] when the value's storage class or content
    /// cannot be converted to `Self`.
    fn from_value(value: Value, column: &str) -> Result<Self>;
}

/// Builds the decoding error for a storage class `T` does not accept.
fn mismatch<T: FromValue>(column: &str, value: &Value) -> Error {
    Error::decode(column, T::TYPE_NAME, format!("unexpected {value:?}"))
}

impl<T: FromValue> FromValue for Option<T> {
    const TYPE_NAME: &'static str = T::TYPE_NAME;

    fn from_value(value: Value, column: &str) -> Result<Self> {
        match value {
            Value::Null => Ok(None),
            other => T::from_value(other, column).map(Some),
        }
    }
}

impl FromValue for Value {
    const TYPE_NAME: &'static str = "Value";

    fn from_value(value: Value, _column: &str) -> Result<Self> {
        Ok(value)
    }
}

/// Implements [`FromValue`] for integer types: integers are range-checked,
/// integral reals are accepted, and text is parsed.
macro_rules! int_from_value {
    ($($t:ty),*) => {$(
        impl FromValue for $t {
            const TYPE_NAME: &'static str = stringify!($t);

            fn from_value(value: Value, column: &str) -> Result<Self> {
                match value {
                    Value::Integer(n) => <$t>::try_from(n)
                        .map_err(|_| Error::decode(column, Self::TYPE_NAME, format!("{n} out of range"))),
                    Value::Real(f) if f.fract() == 0.0 => {
                        // The guard makes the cast exact for any real that
                        // fits; one beyond the i64 range saturates and is
                        // then rejected by the range check below.
                        #[allow(
                            clippy::cast_possible_truncation,
                            reason = "the fract() guard makes the cast exact within range"
                        )]
                        let n = f as i64;
                        <$t>::try_from(n)
                            .map_err(|_| Error::decode(column, Self::TYPE_NAME, format!("{f} out of range")))
                    }
                    Value::Text(s) => s.trim().parse::<$t>()
                        .map_err(|e| Error::decode(column, Self::TYPE_NAME, e)),
                    other => Err(mismatch::<$t>(column, &other)),
                }
            }
        }
    )*};
}
int_from_value!(i8, i16, i32, i64, u8, u16, u32, u64, isize, usize);

impl FromValue for bool {
    const TYPE_NAME: &'static str = "bool";

    /// Decodes any non-zero number as `true`, and the usual textual
    /// spellings case-insensitively; empty text is `false`.
    fn from_value(value: Value, column: &str) -> Result<Self> {
        match value {
            Value::Integer(n) => Ok(n != 0),
            Value::Real(f) => Ok(f != 0.0),
            Value::Text(s) => match s.to_ascii_lowercase().as_str() {
                "1" | "true" | "t" | "yes" | "y" => Ok(true),
                "0" | "false" | "f" | "no" | "n" | "" => Ok(false),
                _ => Err(Error::decode(
                    column,
                    "bool",
                    format!("unexpected text {s:?}"),
                )),
            },
            other => Err(mismatch::<bool>(column, &other)),
        }
    }
}

impl FromValue for f64 {
    const TYPE_NAME: &'static str = "f64";

    fn from_value(value: Value, column: &str) -> Result<Self> {
        match value {
            Value::Real(f) => Ok(f),
            #[allow(
                clippy::cast_precision_loss,
                reason = "an integer column read as a float is expected to round above 2^53"
            )]
            Value::Integer(n) => Ok(n as f64),
            Value::Text(s) => s
                .trim()
                .parse()
                .map_err(|e| Error::decode(column, "f64", e)),
            other => Err(mismatch::<f64>(column, &other)),
        }
    }
}

impl FromValue for f32 {
    const TYPE_NAME: &'static str = "f32";

    fn from_value(value: Value, column: &str) -> Result<Self> {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "narrowing to f32 is what the caller asked for"
        )]
        f64::from_value(value, column).map(|f| f as f32)
    }
}

impl FromValue for String {
    const TYPE_NAME: &'static str = "String";

    /// Decodes text as is, numbers through their `Display` form and blobs
    /// as UTF-8.
    fn from_value(value: Value, column: &str) -> Result<Self> {
        match value {
            Value::Text(s) => Ok(s),
            Value::Integer(n) => Ok(n.to_string()),
            Value::Real(f) => Ok(f.to_string()),
            Value::Blob(b) => String::from_utf8(b).map_err(|e| Error::decode(column, "String", e)),
            other @ Value::Null => Err(mismatch::<String>(column, &other)),
        }
    }
}

impl FromValue for Vec<u8> {
    const TYPE_NAME: &'static str = "Vec<u8>";

    fn from_value(value: Value, column: &str) -> Result<Self> {
        match value {
            Value::Blob(b) => Ok(b),
            Value::Text(s) => Ok(s.into_bytes()),
            other => Err(mismatch::<Vec<u8>>(column, &other)),
        }
    }
}

/// Conversions for the `chrono` date and time types.
///
/// Each type accepts the formats `turso-sql` writes plus the common
/// variants found in databases written by other tools: `T` separators,
/// missing fractions or seconds, RFC 3339 and, for timestamps, Unix
/// seconds stored as integers.
#[cfg(feature = "with-chrono")]
#[cfg_attr(docsrs, doc(cfg(feature = "with-chrono")))]
mod chrono_impls {
    use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Utc};

    use super::{Error, FromValue, Result, Value, mismatch};

    /// Extracts the text of a value, rejecting any other storage class on
    /// behalf of `T`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Decode`] when the value is not text.
    fn text<T: FromValue>(value: Value, column: &str) -> Result<String> {
        match value {
            Value::Text(s) => Ok(s),
            other => Err(mismatch::<T>(column, &other)),
        }
    }

    impl FromValue for NaiveDate {
        const TYPE_NAME: &'static str = "NaiveDate";

        /// Parses `YYYY-MM-DD`, falling back to the date part of any
        /// timestamp format [`NaiveDateTime`] accepts.
        fn from_value(value: Value, column: &str) -> Result<Self> {
            let s = text::<Self>(value, column)?;
            NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
                .or_else(|_| {
                    NaiveDateTime::from_value(Value::Text(s.clone()), column).map(|dt| dt.date())
                })
                .map_err(|e| Error::decode(column, Self::TYPE_NAME, e))
        }
    }

    impl FromValue for NaiveTime {
        const TYPE_NAME: &'static str = "NaiveTime";

        fn from_value(value: Value, column: &str) -> Result<Self> {
            let s = text::<Self>(value, column)?;
            ["%H:%M:%S%.f", "%H:%M:%S", "%H:%M"]
                .iter()
                .find_map(|f| NaiveTime::parse_from_str(s.trim(), f).ok())
                .ok_or_else(|| Error::decode(column, Self::TYPE_NAME, format!("unparsable {s:?}")))
        }
    }

    impl FromValue for NaiveDateTime {
        const TYPE_NAME: &'static str = "NaiveDateTime";

        /// Accepts Unix seconds as an integer, the space- and `T`-separated
        /// forms with or without fractions and seconds, RFC 3339 reduced to
        /// UTC, and a bare date at midnight.
        fn from_value(value: Value, column: &str) -> Result<Self> {
            if let Value::Integer(n) = value {
                return DateTime::from_timestamp(n, 0)
                    .map(|dt| dt.naive_utc())
                    .ok_or_else(|| {
                        Error::decode(column, Self::TYPE_NAME, "timestamp out of range")
                    });
            }
            let s = text::<Self>(value, column)?;
            let t = s.trim();
            [
                "%Y-%m-%d %H:%M:%S%.f",
                "%Y-%m-%d %H:%M:%S",
                "%Y-%m-%dT%H:%M:%S%.f",
                "%Y-%m-%dT%H:%M:%S",
                "%Y-%m-%d %H:%M",
            ]
            .iter()
            .find_map(|f| NaiveDateTime::parse_from_str(t, f).ok())
            .or_else(|| {
                DateTime::parse_from_rfc3339(t)
                    .ok()
                    .map(|dt| dt.naive_utc())
            })
            .or_else(|| {
                NaiveDate::parse_from_str(t, "%Y-%m-%d")
                    .ok()
                    .and_then(|d| d.and_hms_opt(0, 0, 0))
            })
            .ok_or_else(|| Error::decode(column, Self::TYPE_NAME, format!("unparsable {s:?}")))
        }
    }

    impl FromValue for DateTime<FixedOffset> {
        const TYPE_NAME: &'static str = "DateTime<FixedOffset>";

        /// Accepts Unix seconds as an integer, RFC 3339, the space-separated
        /// forms with an offset, and any naive form, which is taken as UTC.
        fn from_value(value: Value, column: &str) -> Result<Self> {
            if let Value::Integer(n) = value {
                return DateTime::from_timestamp(n, 0)
                    .map(|dt| dt.fixed_offset())
                    .ok_or_else(|| {
                        Error::decode(column, Self::TYPE_NAME, "timestamp out of range")
                    });
            }
            let s = text::<Self>(value, column)?;
            let t = s.trim();
            DateTime::parse_from_rfc3339(t)
                .ok()
                .or_else(|| {
                    [
                        "%Y-%m-%d %H:%M:%S%.f%:z",
                        "%Y-%m-%d %H:%M:%S%:z",
                        "%Y-%m-%d %H:%M:%S%.f%#z",
                    ]
                    .iter()
                    .find_map(|f| DateTime::parse_from_str(t, f).ok())
                })
                .or_else(|| {
                    NaiveDateTime::from_value(Value::Text(t.to_owned()), column)
                        .ok()
                        .map(|n| n.and_utc().fixed_offset())
                })
                .ok_or_else(|| Error::decode(column, Self::TYPE_NAME, format!("unparsable {s:?}")))
        }
    }

    impl FromValue for DateTime<Utc> {
        const TYPE_NAME: &'static str = "DateTime<Utc>";

        fn from_value(value: Value, column: &str) -> Result<Self> {
            DateTime::<FixedOffset>::from_value(value, column).map(|dt| dt.with_timezone(&Utc))
        }
    }
}

#[cfg(feature = "with-uuid")]
#[cfg_attr(docsrs, doc(cfg(feature = "with-uuid")))]
impl FromValue for uuid::Uuid {
    const TYPE_NAME: &'static str = "Uuid";

    /// Accepts the hyphenated or simple text forms and 16-byte blobs.
    fn from_value(value: Value, column: &str) -> Result<Self> {
        match value {
            Value::Text(s) => {
                uuid::Uuid::parse_str(s.trim()).map_err(|e| Error::decode(column, "Uuid", e))
            }
            Value::Blob(b) => {
                uuid::Uuid::from_slice(&b).map_err(|e| Error::decode(column, "Uuid", e))
            }
            other => Err(mismatch::<uuid::Uuid>(column, &other)),
        }
    }
}

#[cfg(feature = "with-json")]
#[cfg_attr(docsrs, doc(cfg(feature = "with-json")))]
impl FromValue for serde_json::Value {
    const TYPE_NAME: &'static str = "serde_json::Value";

    /// Parses text and blobs as JSON documents and maps scalars, including
    /// `NULL`, to the corresponding JSON value.
    fn from_value(value: Value, column: &str) -> Result<Self> {
        match value {
            Value::Text(s) => {
                serde_json::from_str(&s).map_err(|e| Error::decode(column, Self::TYPE_NAME, e))
            }
            Value::Blob(b) => {
                serde_json::from_slice(&b).map_err(|e| Error::decode(column, Self::TYPE_NAME, e))
            }
            Value::Integer(n) => Ok(serde_json::Value::from(n)),
            Value::Real(f) => Ok(serde_json::Value::from(f)),
            Value::Null => Ok(serde_json::Value::Null),
        }
    }
}

#[cfg(feature = "with-rust_decimal")]
#[cfg_attr(docsrs, doc(cfg(feature = "with-rust_decimal")))]
impl FromValue for rust_decimal::Decimal {
    const TYPE_NAME: &'static str = "Decimal";

    /// Parses text exactly and converts integers and reals, the latter
    /// with the precision loss inherent to binary floats.
    fn from_value(value: Value, column: &str) -> Result<Self> {
        match value {
            Value::Text(s) => s
                .trim()
                .parse()
                .map_err(|e| Error::decode(column, "Decimal", e)),
            Value::Integer(n) => Ok(rust_decimal::Decimal::from(n)),
            Value::Real(f) => {
                rust_decimal::Decimal::try_from(f).map_err(|e| Error::decode(column, "Decimal", e))
            }
            other => Err(mismatch::<rust_decimal::Decimal>(column, &other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scalars decode leniently across storage classes, out-of-range
    /// integers are rejected, and only `Option<T>` accepts `NULL`.
    #[test]
    fn lenient_scalars() {
        assert!(bool::from_value(Value::Integer(1), "c").unwrap());
        assert_eq!(i32::from_value(Value::Integer(7), "c").unwrap(), 7);
        assert!(i8::from_value(Value::Integer(300), "c").is_err());
        assert!((f64::from_value(Value::Integer(2), "c").unwrap() - 2.0).abs() < f64::EPSILON);
        assert_eq!(Option::<i64>::from_value(Value::Null, "c").unwrap(), None);
        assert!(i64::from_value(Value::Null, "c").is_err());
        assert_eq!(String::from_value(Value::Integer(5), "c").unwrap(), "5");
    }

    /// Timestamps decode from the stored text form, from RFC 3339 with an
    /// offset normalised to UTC, and from Unix seconds.
    #[cfg(feature = "with-chrono")]
    #[test]
    fn chrono() {
        use chrono::{DateTime, NaiveDateTime, Utc};
        let dt =
            NaiveDateTime::from_value(Value::Text("2024-01-02 03:04:05.123".into()), "c").unwrap();
        assert_eq!(dt.to_string(), "2024-01-02 03:04:05.123");
        let utc = DateTime::<Utc>::from_value(Value::Text("2024-01-02T03:04:05+02:00".into()), "c")
            .unwrap();
        assert_eq!(utc.to_rfc3339(), "2024-01-02T01:04:05+00:00");
        let from_int = DateTime::<Utc>::from_value(Value::Integer(0), "c").unwrap();
        assert_eq!(from_int.timestamp(), 0);
    }
}
