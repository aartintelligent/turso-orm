//! The `DeriveRelation` expansion.
//!
//! A `Relation` enum lists the other entities one entity is related to, one
//! variant each. The derive turns every variant into a `RelationDef` arm of
//! `RelationTrait::def` and, for the first variant naming a given target,
//! into a `Related<Target>` impl on the local `Entity`, which is what
//! `find_related`, `inner_join` and the loaders require. A second variant
//! to the same target — two foreign keys to `user`, or both sides of a
//! self-reference — keeps its `def()` and is used through
//! `Select::join(kind, &Relation::X.def())` and `Select::related_to`,
//! because Rust allows one `Related<Target>` impl per entity.
//!
//! The columns of a `has_many` or `has_one` relation are usually omitted:
//! the owning side already spelled them out in its `belongs_to`, so the
//! derive resolves them at runtime by taking the reverse of the relation the
//! target declares back to us. Only `belongs_to` has to name its columns,
//! because it is the side that holds the foreign key.
//!
//! A many-to-many relation is a `has_many` with `via = "junction::Entity"`:
//! the junction entity declares a `belongs_to` towards each side, and the
//! derive assembles `Related::via` (this entity to the junction) and
//! `Related::to` (the junction to the target) from those two.

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Data, DeriveInput};

use crate::attrs;

/// Expands a `Relation` enum into `Iterable`, `RelationTrait` and `Related<R>` impls.
///
/// # Errors
///
/// Returns a compile error when the input is not an enum, when a variant
/// lacks `belongs_to`, `has_many` or `has_one`, when a variant carries an
/// unknown key, when a target or column path does not parse, when `via` is
/// combined with explicit columns or with `belongs_to`, or when an
/// `on_delete` / `on_update` value is not a known foreign-key action.
pub(crate) fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let ident = &input.ident;
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            ident,
            "DeriveRelation only supports enums",
        ));
    };

    let mut variants = Vec::new();
    let mut def_arms = Vec::new();
    let mut related_impls = Vec::new();
    let mut seen_targets: Vec<String> = Vec::new();

    for v in &data.variants {
        let vi = &v.ident;
        variants.push(quote! { Self::#vi });

        let mut kind: Option<(&str, String)> = None;
        let mut from: Vec<syn::Path> = Vec::new();
        let mut to: Vec<syn::Path> = Vec::new();
        let mut via: Option<syn::Path> = None;
        let mut on_delete = None;
        let mut on_update = None;
        let mut skip_fk = false;

        for item in attrs::parse(&v.attrs)? {
            match item.key.as_str() {
                "belongs_to" | "has_many" | "has_one" => {
                    kind = Some((
                        match item.key.as_str() {
                            "belongs_to" => "belongs_to",
                            "has_many" => "has_many",
                            _ => "has_one",
                        },
                        item.str_value()?,
                    ));
                }
                "from" => from.push(syn::parse_str(&item.str_value()?)?),
                "to" => to.push(syn::parse_str(&item.str_value()?)?),
                "via" => via = Some(syn::parse_str(&item.str_value()?)?),
                "on_delete" => on_delete = Some(fk_action(&item.str_value()?, item.span)?),
                "on_update" => on_update = Some(fk_action(&item.str_value()?, item.span)?),
                "skip_fk" => skip_fk = item.bool_value()?,
                other => {
                    return Err(syn::Error::new(
                        item.span,
                        format!("unknown relation attribute `{other}`"),
                    ));
                }
            }
        }

        let Some((kind, target)) = kind else {
            return Err(syn::Error::new_spanned(
                vi,
                "relation variants need `belongs_to`, `has_many` or `has_one`",
            ));
        };
        let target: syn::Path = syn::parse_str(&target)?;
        // The kind doubles as the name of the `EntityTrait` builder method.
        let builder = syn::Ident::new(kind, vi.span());
        let from_calls = from.iter().map(|p| quote! { .from(#p) });
        let tos = to.iter().map(|p| quote! { .to(#p) });
        let on_delete = on_delete.map(|a| quote! { .on_delete(#a) });
        let on_update = on_update.map(|a| quote! { .on_update(#a) });
        let skip = skip_fk.then(|| quote! { .skip_fk() });

        let (def, to_impl, via_impl) = if let Some(junction) = via {
            if kind == "belongs_to" || !from.is_empty() || !to.is_empty() {
                return Err(syn::Error::new_spanned(
                    vi,
                    "`via` goes with `has_many` and infers its columns from the junction entity",
                ));
            }
            // Both hops come from the `belongs_to` relations the junction
            // declares: this entity to the junction is the reverse of the
            // junction's relation back to us, and the junction to the target
            // is the junction's own relation.
            (
                quote! {
                    <#junction as ::turso_orm::entity::Related<Entity>>::to().rev()
                },
                quote! {
                    <#junction as ::turso_orm::entity::Related<#target>>::to()
                },
                quote! {
                    fn via() -> ::core::option::Option<::turso_orm::entity::RelationDef> {
                        ::core::option::Option::Some(::turso_orm::entity::RelationTrait::def(&#ident::#vi))
                    }
                },
            )
        } else if from.is_empty() && kind != "belongs_to" {
            // A `has_many` / `has_one` without explicit columns is inferred
            // from the reverse relation the target declares back to this
            // entity, so the columns are spelled out once, on the owning side.
            (
                quote! {
                    <#target as ::turso_orm::entity::Related<Entity>>::to().rev()
                },
                quote! { ::turso_orm::entity::RelationTrait::def(&#ident::#vi) },
                quote! {},
            )
        } else {
            (
                quote! {
                    <Entity as ::turso_orm::entity::EntityTrait>::#builder(#target)
                        #(#from_calls)* #(#tos)* #on_delete #on_update #skip
                        .into()
                },
                quote! { ::turso_orm::entity::RelationTrait::def(&#ident::#vi) },
                quote! {},
            )
        };

        def_arms.push(quote! { Self::#vi => #def, });

        // One `Related<Target>` impl per target: the first variant wins and
        // the others stay reachable through their `def()`.
        let target_key = target.to_token_stream().to_string();
        if !seen_targets.contains(&target_key) {
            seen_targets.push(target_key);
            related_impls.push(quote! {
                impl ::turso_orm::entity::Related<#target> for Entity {
                    fn to() -> ::turso_orm::entity::RelationDef {
                        #to_impl
                    }

                    #via_impl
                }
            });
        }
    }

    // An empty enum has no arms; `match *self {}` is the only body that
    // type-checks for an uninhabited type.
    let def_body = if def_arms.is_empty() {
        quote! { match *self {} }
    } else {
        quote! { match self { #(#def_arms)* } }
    };

    Ok(quote! {
        impl ::turso_orm::entity::Iterable for #ident {
            const ALL: &'static [Self] = &[#(#variants),*];
        }

        impl ::turso_orm::entity::RelationTrait for #ident {
            fn def(&self) -> ::turso_orm::entity::RelationDef {
                #def_body
            }
        }

        #(#related_impls)*
    })
}

/// Maps an `on_delete` / `on_update` attribute value to a `ForeignKeyAction` path.
///
/// # Errors
///
/// Returns an error at `span` when `name` is not one of the five SQLite
/// referential actions.
fn fk_action(name: &str, span: proc_macro2::Span) -> syn::Result<TokenStream> {
    let variant = match name {
        "Cascade" => "Cascade",
        "SetNull" => "SetNull",
        "SetDefault" => "SetDefault",
        "Restrict" => "Restrict",
        "NoAction" => "NoAction",
        _ => {
            return Err(syn::Error::new(
                span,
                "expected one of Cascade, SetNull, SetDefault, Restrict, NoAction",
            ));
        }
    };
    let v = syn::Ident::new(variant, span);
    Ok(quote! { ::turso_orm::sql::ForeignKeyAction::#v })
}
