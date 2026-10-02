//! Parsing of `#[turso(...)]` attributes into a flat list of [`Item`]s.
//!
//! Every derive accepts the same attribute shape — a comma-separated list of
//! bare keys (`primary_key`) and `key = value` pairs (`table_name = "user"`)
//! — so parsing is done once here and each derive only interprets the keys
//! it knows. Keeping the result as plain strings and expressions rather than
//! a typed struct per derive lets the derives report unknown keys with the
//! span of the key itself.

use syn::spanned::Spanned;
use syn::{Attribute, Expr, Lit, Meta, Token, punctuated::Punctuated};

/// One `key` or `key = value` item of a `#[turso(...)]` attribute.
#[derive(Debug)]
pub(crate) struct Item {
    /// The key, as written.
    pub(crate) key: String,
    /// The value after `=`, or `None` for a bare key.
    pub(crate) value: Option<Expr>,
    /// The span of the key, used for error reporting.
    pub(crate) span: proc_macro2::Span,
}

impl Item {
    /// Reads the value as a string literal.
    ///
    /// # Errors
    ///
    /// Returns an error at the key's span when the value is missing or is
    /// not a string literal.
    pub(crate) fn str_value(&self) -> syn::Result<String> {
        match &self.value {
            Some(Expr::Lit(syn::ExprLit {
                lit: Lit::Str(s), ..
            })) => Ok(s.value()),
            _ => Err(syn::Error::new(
                self.span,
                format!("`{}` expects a string literal", self.key),
            )),
        }
    }

    /// Reads the value as a boolean, treating a bare key as `true`.
    ///
    /// # Errors
    ///
    /// Returns an error at the key's span when the value is present but is
    /// not a boolean literal.
    pub(crate) fn bool_value(&self) -> syn::Result<bool> {
        match &self.value {
            None => Ok(true),
            Some(Expr::Lit(syn::ExprLit {
                lit: Lit::Bool(b), ..
            })) => Ok(b.value),
            _ => Err(syn::Error::new(
                self.span,
                format!("`{}` expects a boolean", self.key),
            )),
        }
    }
}

/// Parses every `#[turso(...)]` attribute in `attrs`, ignoring other attributes.
///
/// # Errors
///
/// Returns an error when the attribute body is not a comma-separated list of
/// `key` or `key = value` items, when a key is not a plain identifier, or
/// when an item is a nested list such as `key(...)`.
pub(crate) fn parse(attrs: &[Attribute]) -> syn::Result<Vec<Item>> {
    let mut items = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("turso") {
            continue;
        }
        let nested = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        for meta in nested {
            let span = meta_span(&meta);
            match meta {
                Meta::Path(p) => items.push(Item {
                    key: path_name(&p)?,
                    value: None,
                    span,
                }),
                Meta::NameValue(nv) => items.push(Item {
                    key: path_name(&nv.path)?,
                    value: Some(nv.value),
                    span,
                }),
                Meta::List(l) => {
                    return Err(syn::Error::new(
                        l.path.span(),
                        "nested lists are not supported",
                    ));
                }
            }
        }
    }
    Ok(items)
}

/// The span of an item's key, so errors point at the key rather than the whole attribute.
fn meta_span(meta: &Meta) -> proc_macro2::Span {
    meta.path().span()
}

/// The single identifier of a key path.
///
/// # Errors
///
/// Returns an error when the path has more than one segment, such as `a::b`.
fn path_name(path: &syn::Path) -> syn::Result<String> {
    path.get_ident()
        .map(ToString::to_string)
        .ok_or_else(|| syn::Error::new(path.span(), "expected an identifier"))
}
