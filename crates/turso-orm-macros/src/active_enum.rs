//! The `DeriveActiveEnum` expansion.
//!
//! A fieldless enum with `#[turso(rs_type = "String")]` or an integer type
//! becomes a column type: the derive maps each variant to the value given
//! by `string_value` or `num_value` (the `snake_case` variant name by
//! default for text) and emits `ActiveEnum`, `From<Enum> for Value`,
//! `FromValue` and `TursoType`, which together let the enum be a model
//! field, a bound condition value and a DDL column.

use heck::ToSnakeCase;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Expr, Lit};

use crate::attrs;

/// Expands a fieldless enum into the column-type impls.
///
/// # Errors
///
/// Returns a compile error when the input is not an enum, when a variant
/// has fields, when `rs_type` is missing or is neither `String` nor an
/// integer type, when a variant value has the wrong literal type for
/// `rs_type`, or when an attribute key is unknown.
pub(crate) fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let ident = &input.ident;
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            ident,
            "DeriveActiveEnum only supports enums",
        ));
    };

    let mut rs_type: Option<syn::Type> = None;
    for item in attrs::parse(&input.attrs)? {
        match item.key.as_str() {
            "rs_type" => rs_type = Some(syn::parse_str(&item.str_value()?)?),
            other => {
                return Err(syn::Error::new(
                    item.span,
                    format!("unknown active enum attribute `{other}`"),
                ));
            }
        }
    }
    let rs_type = rs_type.ok_or_else(|| {
        syn::Error::new_spanned(
            ident,
            "missing `#[turso(rs_type = \"String\")]` or an integer type",
        )
    })?;
    let is_string = matches!(&rs_type, syn::Type::Path(p) if p.path.is_ident("String"));
    let column_type = if is_string {
        quote! { ::turso_orm::sql::ColumnType::Text }
    } else {
        quote! { ::turso_orm::sql::ColumnType::Integer }
    };

    let mut variants = Vec::new();
    let mut to_arms = Vec::new();
    let mut from_arms = Vec::new();
    for v in &data.variants {
        let vi = &v.ident;
        if !matches!(v.fields, syn::Fields::Unit) {
            return Err(syn::Error::new_spanned(
                vi,
                "DeriveActiveEnum variants cannot have fields",
            ));
        }
        let mut value: Option<Expr> = None;
        for item in attrs::parse(&v.attrs)? {
            match item.key.as_str() {
                "string_value" if is_string => {
                    let s = item.str_value()?;
                    value = Some(syn::parse_quote! { #s });
                }
                "num_value" if !is_string => match item.value {
                    Some(Expr::Lit(syn::ExprLit {
                        lit: Lit::Int(n), ..
                    })) => value = Some(syn::parse_quote! { #n }),
                    _ => {
                        return Err(syn::Error::new(
                            item.span,
                            "`num_value` expects an integer literal",
                        ));
                    }
                },
                "string_value" | "num_value" => {
                    return Err(syn::Error::new(
                        item.span,
                        format!("`{}` does not match `rs_type`", item.key),
                    ));
                }
                other => {
                    return Err(syn::Error::new(
                        item.span,
                        format!("unknown active enum variant attribute `{other}`"),
                    ));
                }
            }
        }
        let value = match value {
            Some(v) => v,
            None if is_string => {
                let s = vi.to_string().to_snake_case();
                syn::parse_quote! { #s }
            }
            None => {
                return Err(syn::Error::new_spanned(
                    vi,
                    "integer-backed variants need `#[turso(num_value = ...)]`",
                ));
            }
        };
        variants.push(quote! { Self::#vi });
        // Text values are owned `String`s at runtime, so the match on the
        // way in compares through `as_str`.
        if is_string {
            to_arms.push(quote! { Self::#vi => ::std::string::String::from(#value), });
            from_arms.push(quote! { #value => ::core::result::Result::Ok(Self::#vi), });
        } else {
            to_arms.push(quote! { Self::#vi => #value, });
            from_arms.push(quote! { #value => ::core::result::Result::Ok(Self::#vi), });
        }
    }

    let name = ident.to_string();
    let scrutinee = if is_string {
        quote! { value.as_str() }
    } else {
        quote! { *value }
    };
    let def_body = if from_arms.is_empty() {
        quote! { match #scrutinee { _ => ::core::result::Result::Err(::turso_orm::DbErr::Type(::std::format!("no variant of {} for {:?}", #name, value))) } }
    } else {
        quote! {
            match #scrutinee {
                #(#from_arms)*
                _ => ::core::result::Result::Err(::turso_orm::DbErr::Type(::std::format!("no variant of {} for {:?}", #name, value))),
            }
        }
    };
    let to_body = if to_arms.is_empty() {
        quote! { match *self {} }
    } else {
        quote! { match self { #(#to_arms)* } }
    };

    Ok(quote! {
        impl ::turso_orm::entity::ActiveEnum for #ident {
            type Value = #rs_type;
            const COLUMN_TYPE: ::turso_orm::sql::ColumnType = #column_type;
            const NAME: &'static str = #name;

            fn to_value(&self) -> #rs_type {
                #to_body
            }

            fn try_from_value(value: &#rs_type) -> ::turso_orm::Result<Self> {
                #def_body
            }

            fn values() -> ::std::vec::Vec<Self> {
                ::std::vec![#(#variants),*]
            }
        }

        impl ::core::convert::From<#ident> for ::turso_orm::Value {
            fn from(v: #ident) -> Self {
                ::turso_orm::entity::ActiveEnum::into_value(v)
            }
        }

        impl ::turso_orm::__private::FromValue for #ident {
            const TYPE_NAME: &'static str = #name;

            fn from_value(value: ::turso_orm::Value, column: &str) -> ::turso_orm::__private::DriverResult<Self> {
                let inner = <#rs_type as ::turso_orm::__private::FromValue>::from_value(value, column)?;
                <Self as ::turso_orm::entity::ActiveEnum>::try_from_value(&inner).map_err(|e| {
                    ::turso_orm::__private::DriverError::decode(column, #name, e)
                })
            }
        }

        impl ::turso_orm::types::TursoType for #ident {
            const COLUMN_TYPE: ::turso_orm::sql::ColumnType = #column_type;
        }
    })
}
