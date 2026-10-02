//! The `DeriveIden` expansion and the shared identifier impls.
//!
//! Three traits describe a static identifier: `IdenStatic` from the entity
//! layer, and `Iden` and `IntoIden` from the SQL layer. They always travel
//! together, so [`impl_iden`] emits all three from one `as_str` body and is
//! reused by the entity derive for its `Entity`, `Column` and `PrimaryKey`
//! types. `DeriveIden` itself is the standalone form for user-written unit
//! structs and fieldless enums, with `snake_case` names by default.

use heck::ToSnakeCase;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput};

use crate::attrs;

/// Reads an explicit `#[turso(iden = "...")]` name, if present.
///
/// # Errors
///
/// Returns an error when the attributes do not parse or when `iden` is not a
/// string literal.
fn override_name(attrs: &[syn::Attribute]) -> syn::Result<Option<String>> {
    for item in attrs::parse(attrs)? {
        if item.key == "iden" {
            return item.str_value().map(Some);
        }
    }
    Ok(None)
}

/// Expands a unit struct or a fieldless enum into the identifier impls.
///
/// # Errors
///
/// Returns a compile error when the input is a union, or when a
/// struct-level `iden` attribute does not parse. A malformed `iden` on an
/// enum variant falls back to the `snake_case` name instead of failing.
pub(crate) fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let ident = &input.ident;
    let body = match &input.data {
        Data::Struct(_) => {
            let name =
                override_name(&input.attrs)?.unwrap_or_else(|| ident.to_string().to_snake_case());
            quote! { #name }
        }
        Data::Enum(e) => {
            let arms = e.variants.iter().map(|v| {
                let vi = &v.ident;
                let name = override_name(&v.attrs)
                    .unwrap_or(None)
                    .unwrap_or_else(|| vi.to_string().to_snake_case());
                quote! { Self::#vi => #name, }
            });
            quote! { match self { #(#arms)* } }
        }
        Data::Union(_) => return Err(syn::Error::new_spanned(ident, "unions are not supported")),
    };
    Ok(impl_iden(ident, body))
}

/// Emits the three identifier impls every table or column type needs.
///
/// `as_str_body` is the expression body of `IdenStatic::as_str`; the SQL
/// layer's `Iden` and `IntoIden` impls delegate to it so that the name is
/// defined exactly once.
pub(crate) fn impl_iden(ident: &syn::Ident, as_str_body: TokenStream) -> TokenStream {
    quote! {
        impl ::turso_orm::entity::IdenStatic for #ident {
            fn as_str(&self) -> &'static str {
                #as_str_body
            }
        }

        impl ::turso_orm::sql::Iden for #ident {
            fn as_str(&self) -> &str {
                <Self as ::turso_orm::entity::IdenStatic>::as_str(self)
            }
        }

        impl ::turso_orm::__private::IntoIden for #ident {
            fn into_iden(self) -> ::turso_orm::__private::Ident {
                ::turso_orm::__private::Ident::new_static(<Self as ::turso_orm::entity::IdenStatic>::as_str(&self))
            }
        }
    }
}
