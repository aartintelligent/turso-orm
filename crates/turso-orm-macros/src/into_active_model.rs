//! The `DeriveIntoActiveModel` expansion.
//!
//! A request or form struct rarely matches the model one for one: it
//! carries a subset of the columns and leaves the rest to defaults. The
//! derive turns such a struct into the entity's active model field by
//! field, through `IntoActiveValue`, so that a plain field becomes `Set`
//! and an `Option` field becomes `Set` or `NotSet`. The target is the
//! `ActiveModel` in scope unless `#[turso(active_model = "path::ActiveModel")]`
//! names another; `#[turso(ignore)]` leaves a field out.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::attrs;

/// Expands a struct with named fields into an `IntoActiveModel` impl.
///
/// # Errors
///
/// Returns a compile error when the input is not a struct with named
/// fields, when `active_model` does not parse as a path, or when an
/// attribute key is unknown.
pub(crate) fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let ident = &input.ident;
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            ident,
            "DeriveIntoActiveModel only supports structs",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            ident,
            "DeriveIntoActiveModel needs named fields",
        ));
    };

    let mut active_model: syn::Path = syn::parse_quote! { ActiveModel };
    for item in attrs::parse(&input.attrs)? {
        match item.key.as_str() {
            "active_model" => active_model = syn::parse_str(&item.str_value()?)?,
            other => {
                return Err(syn::Error::new(
                    item.span,
                    format!("unknown into-active-model attribute `{other}`"),
                ));
            }
        }
    }

    let mut sets = Vec::new();
    for f in &fields.named {
        // Unnamed fields were rejected above, so this `expect` is unreachable.
        let name = f.ident.as_ref().expect("named");
        let mut ignore = false;
        for item in attrs::parse(&f.attrs)? {
            match item.key.as_str() {
                "ignore" => ignore = item.bool_value()?,
                other => {
                    return Err(syn::Error::new(
                        item.span,
                        format!("unknown into-active-model field attribute `{other}`"),
                    ));
                }
            }
        }
        if !ignore {
            sets.push(quote! {
                am.#name = ::turso_orm::entity::IntoActiveValue::into_active_value(self.#name);
            });
        }
    }

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics ::turso_orm::entity::IntoActiveModel<#active_model> for #ident #ty_generics #where_clause {
            fn into_active_model(self) -> #active_model {
                let mut am = <#active_model as ::core::default::Default>::default();
                #(#sets)*
                am
            }
        }
    })
}
