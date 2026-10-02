//! Identifiers for tables, columns and aliases, modeled by [`Ident`].
//!
//! Every name that reaches the writer goes through this module so that it is
//! rendered double-quoted and with embedded quotes doubled; that is what makes
//! reserved words, mixed case and user-supplied names safe to interpolate.
//! The module owns the identifier types and their conversions only — it does
//! not know how an identifier is rendered, which is the writer's business.
//!
//! - [`Iden`]: the trait a type implements to name something, implemented for
//!   string types here and for column and table enums through the derive
//!   macros of `turso-orm`;
//! - [`Ident`]: an owned identifier backed by a `Cow` so static names never
//!   allocate;
//! - [`IntoIden`]: the conversion builders accept, so a `&'static str`, a
//!   `String` or a derived enum can be passed interchangeably;
//! - [`TableRef`] and [`ColumnRef`]: a table with its optional alias and a
//!   column optionally qualified by table.

use std::borrow::Cow;
use std::fmt;

/// Something that names a table, column or alias.
///
/// Implemented for string types and, through the derive macros in
/// `turso-orm`, for column and table enums.
pub trait Iden {
    /// The unquoted identifier.
    fn as_str(&self) -> &str;
}

impl Iden for &str {
    fn as_str(&self) -> &str {
        self
    }
}

impl Iden for String {
    fn as_str(&self) -> &str {
        self
    }
}

impl Iden for Ident {
    fn as_str(&self) -> &str {
        &self.0
    }
}

/// An owned identifier.
///
/// The backing `Cow` lets names known at compile time be carried without an
/// allocation, which matters because derived entity code names every column
/// on every query.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Ident(pub Cow<'static, str>);

impl Ident {
    /// Builds an identifier from a static string without allocating.
    pub const fn new_static(name: &'static str) -> Self {
        Ident(Cow::Borrowed(name))
    }

    /// The unquoted name.
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Conversion into an [`Ident`].
///
/// Builders take `impl IntoIden` so that callers can pass a `&'static str`, a
/// `String`, an existing [`Ident`] or a reference to a derived identifier
/// enum without converting by hand.
pub trait IntoIden {
    /// Converts into an owned identifier.
    fn into_iden(self) -> Ident;
}

impl IntoIden for Ident {
    fn into_iden(self) -> Ident {
        self
    }
}

impl IntoIden for &'static str {
    fn into_iden(self) -> Ident {
        Ident(Cow::Borrowed(self))
    }
}

impl IntoIden for String {
    fn into_iden(self) -> Ident {
        Ident(Cow::Owned(self))
    }
}

impl<T: Iden + ?Sized> IntoIden for &T
where
    T: IdenOwnedMarker,
{
    fn into_iden(self) -> Ident {
        Ident(Cow::Owned(self.as_str().to_owned()))
    }
}

/// Marker for reference conversions of custom [`Iden`] types.
///
/// A blanket `impl IntoIden for &T where T: Iden` would overlap with the
/// `&'static str` implementation, so custom identifier types opt in through
/// this marker instead; the derive macros of `turso-orm` implement it.
pub trait IdenOwnedMarker {}

impl From<&'static str> for Ident {
    fn from(v: &'static str) -> Self {
        v.into_iden()
    }
}

impl From<String> for Ident {
    fn from(v: String) -> Self {
        v.into_iden()
    }
}

/// A table reference with an optional alias.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableRef {
    /// The table name.
    pub name: Ident,
    /// The `AS alias` part, when the table is aliased.
    pub alias: Option<Ident>,
}

impl TableRef {
    /// A table reference without alias.
    pub fn new(name: impl IntoIden) -> Self {
        Self {
            name: name.into_iden(),
            alias: None,
        }
    }

    /// Sets the alias.
    #[must_use]
    pub fn alias(mut self, alias: impl IntoIden) -> Self {
        self.alias = Some(alias.into_iden());
        self
    }

    /// The identifier other clauses should use to refer to this table.
    ///
    /// Once a table is aliased, SQL requires every qualified column to use
    /// the alias rather than the original name.
    pub fn reference(&self) -> &Ident {
        self.alias.as_ref().unwrap_or(&self.name)
    }
}

impl<T: IntoIden> From<T> for TableRef {
    fn from(name: T) -> Self {
        TableRef::new(name)
    }
}

/// A column reference, optionally qualified by table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColumnRef {
    /// A bare `column`.
    Column(Ident),
    /// A qualified `table.column`.
    TableColumn(Ident, Ident),
    /// The `*` wildcard.
    Asterisk,
    /// The `table.*` wildcard.
    TableAsterisk(Ident),
}

impl<T: IntoIden> From<T> for ColumnRef {
    fn from(name: T) -> Self {
        ColumnRef::Column(name.into_iden())
    }
}

impl<T: IntoIden, C: IntoIden> From<(T, C)> for ColumnRef {
    fn from((table, column): (T, C)) -> Self {
        ColumnRef::TableColumn(table.into_iden(), column.into_iden())
    }
}
