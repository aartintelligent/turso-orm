//! The `DerivePartialModel` expansion.
//!
//! A partial model is a struct that reads some columns of an entity, or
//! expressions over them, under its own field names. The struct names its
//! entity with `#[turso(entity = "path::Entity")]`; each field reads the
//! column whose variant matches the field name in `UpperCamelCase`, or the
//! one named with `from_col = "Variant"`, or the expression given with
//! `from_expr = "..."`. The derive emits `PartialModelTrait::select_cols`,
//! which adds one aliased item per field, and `FromQueryResult`, which
//! reads each field back by that alias.

use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields};

use crate::attrs;

/// Expands a struct with named fields into the partial-model impls.
///
/// # Errors
///
/// Returns a compile error when the input is not a struct with named
/// fields, when `entity` is missing, when an attribute key is unknown, or
/// when a `from_expr` value does not parse as an expression.
pub(crate) fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let ident = &input.ident;
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            ident,
            "DerivePartialModel only supports structs",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            ident,
            "DerivePartialModel needs named fields",
        ));
    };

    let mut entity: Option<syn::Path> = None;
    for item in attrs::parse(&input.attrs)? {
        match item.key.as_str() {
            "entity" => entity = Some(syn::parse_str(&item.str_value()?)?),
            other => {
                return Err(syn::Error::new(
                    item.span,
                    format!("unknown partial model attribute `{other}`"),
                ));
            }
        }
    }
    let entity = entity.ok_or_else(|| {
        syn::Error::new_spanned(ident, "missing `#[turso(entity = \"path::Entity\")]`")
    })?;

    let mut selects = Vec::new();
    let mut reads = Vec::new();
    for f in &fields.named {
        // Unnamed fields were rejected above, so this `expect` is unreachable.
        let name = f.ident.as_ref().expect("named");
        let ty = &f.ty;
        let alias = name.to_string();
        let mut source: Option<TokenStream> = None;
        for item in attrs::parse(&f.attrs)? {
            match item.key.as_str() {
                "from_col" => {
                    let variant = format_ident!("{}", item.str_value()?);
                    source = Some(column_expr(&entity, &variant));
                }
                "from_expr" => {
                    let expr: syn::Expr = syn::parse_str(&item.str_value()?)?;
                    source = Some(quote! { #expr });
                }
                other => {
                    return Err(syn::Error::new(
                        item.span,
                        format!("unknown partial model field attribute `{other}`"),
                    ));
                }
            }
        }
        let source = source.unwrap_or_else(|| {
            let variant = format_ident!("{}", alias.to_upper_camel_case());
            column_expr(&entity, &variant)
        });
        selects.push(quote! { let select = select.expr_as(#source, #alias); });
        reads.push(quote! {
            #name: ::turso_orm::__private::get_field::<#ty>(row, prefix, #alias)?,
        });
    }

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics ::turso_orm::entity::FromQueryResult for #ident #ty_generics #where_clause {
            fn from_query_result(row: &::turso_orm::__private::Row, prefix: &str) -> ::turso_orm::Result<Self> {
                Ok(Self { #(#reads)* })
            }
        }

        impl #impl_generics ::turso_orm::entity::PartialModelTrait for #ident #ty_generics #where_clause {
            fn select_cols(select: ::turso_orm::sql::Select) -> ::turso_orm::sql::Select {
                #(#selects)*
                select
            }
        }
    })
}

/// The qualified column expression of `variant` on `entity`.
fn column_expr(entity: &syn::Path, variant: &syn::Ident) -> TokenStream {
    quote! {
        ::turso_orm::entity::ColumnTrait::into_expr(
            <#entity as ::turso_orm::entity::EntityTrait>::Column::#variant
        )
    }
}
