//! The `FromQueryResult` expansion.
//!
//! Custom projection structs — the target of `Select::into_model` — need the
//! same row decoding as models but none of the entity machinery, so this
//! derive emits only the `FromQueryResult` impl. Fields are read by column
//! name through `get_field`, which honours the `prefix` argument, so a
//! derived struct can also stand in for either side of a `find_also_related`
//! row.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::attrs;

/// Expands a struct with named fields into a `FromQueryResult` impl.
///
/// # Errors
///
/// Returns a compile error when the input is not a struct with named
/// fields, when a field attribute does not parse, or when `column_name` is
/// not a string literal.
pub(crate) fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let ident = &input.ident;
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            ident,
            "FromQueryResult only supports structs",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            ident,
            "FromQueryResult needs named fields",
        ));
    };
    let mut reads = Vec::new();
    for f in &fields.named {
        // Unnamed fields were rejected above, so this `expect` is unreachable.
        let name = f.ident.as_ref().expect("named");
        let ty = &f.ty;
        let mut column = name.to_string();
        for item in attrs::parse(&f.attrs)? {
            if item.key == "column_name" {
                column = item.str_value()?;
            }
        }
        reads.push(quote! {
            #name: ::turso_orm::__private::get_field::<#ty>(row, prefix, #column)?,
        });
    }
    // Generics are forwarded so that a projection struct may be generic
    // over a field type.
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics ::turso_orm::entity::FromQueryResult for #ident #ty_generics #where_clause {
            fn from_query_result(row: &::turso_orm::__private::Row, prefix: &str) -> ::turso_orm::Result<Self> {
                Ok(Self { #(#reads)* })
            }
        }
    })
}
