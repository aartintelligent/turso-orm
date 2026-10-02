//! Macros: the derive macros that generate entity, relation, identifier and row-decoding impls for turso-orm.
//!
//! The macros are re-exported by `turso_orm` behind its `macros` feature
//! and by `turso_orm_migration`; they are not meant to be used from this
//! crate directly, because the code they expand to refers to
//! `::turso_orm::...` paths and to the `__private` module that crate
//! exposes for generated code only.
//!
//! Each derive lives in its own module with a single `expand` function that
//! takes the parsed input and returns either the generated tokens or a
//! `syn::Error` carrying the span of the offending item, so that users see
//! the error on their own code rather than inside the macro. Attribute
//! parsing is shared through the `attrs` module.
//!
//! - `DeriveEntityModel` expands a `Model` struct into `Entity`, `Column`,
//!   `PrimaryKey` and `ActiveModel` plus their trait impls.
//! - `DeriveRelation` expands a `Relation` enum into `RelationTrait` and
//!   `Related<R>` impls, junction tables included.
//! - `DeriveActiveEnum` makes a fieldless enum a column type.
//! - `DerivePartialModel` makes a struct a self-selecting projection.
//! - `DeriveIntoActiveModel` converts a plain struct into an active model.
//! - `DeriveIden` implements the identifier traits on a unit struct or a
//!   fieldless enum.
//! - `FromQueryResult` implements row decoding for a plain struct.
//! - `DeriveMigrationName` names a migration after its module.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    reason = "proc-macro expansion code is naturally long and passes syn trees by value"
)]

mod active_enum;
mod attrs;
mod entity;
mod from_query_result;
mod iden;
mod into_active_model;
mod partial_model;
mod relation;

use proc_macro::TokenStream;

/// Derives an entity from a `Model` struct.
///
/// Generates `Entity`, `Column`, `PrimaryKey` and `ActiveModel` next to the
/// model. Field attributes: `#[turso(primary_key)]`,
/// `#[turso(auto_increment = false)]`, `#[turso(column_name = "...")]`,
/// `#[turso(unique)]`, `#[turso(indexed)]`, `#[turso(default_value = ...)]`,
/// `#[turso(ignore)]`. Struct attribute: `#[turso(table_name = "...")]`.
#[proc_macro_derive(DeriveEntityModel, attributes(turso))]
pub fn derive_entity_model(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    entity::expand(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derives `RelationTrait` and `Related<R>` impls from a `Relation` enum.
///
/// Variant attributes: `#[turso(belongs_to = "super::user::Entity", from = "Column::UserId", to = "super::user::Column::Id")]`,
/// `#[turso(has_many = "super::post::Entity")]`, `#[turso(has_one = "...")]`,
/// `#[turso(has_many = "super::tag::Entity", via = "super::post_tag::Entity")]`
/// for a many-to-many relation, plus optional `on_delete = "Cascade"` /
/// `on_update = "..."` / `skip_fk`. The first variant naming a target
/// provides the `Related<Target>` impl; further variants to the same target
/// are used through their `def()`.
#[proc_macro_derive(DeriveRelation, attributes(turso))]
pub fn derive_relation(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    relation::expand(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derives `IdenStatic` and the SQL identifier traits for a unit struct or
/// a fieldless enum.
///
/// Names are `snake_case` unless overridden with `#[turso(iden = "...")]`.
#[proc_macro_derive(DeriveIden, attributes(turso))]
pub fn derive_iden(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    iden::expand(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derives `FromQueryResult` for a plain struct.
///
/// Every field is read from the column of the same name, or from the one
/// given with `#[turso(column_name = "...")]`.
#[proc_macro_derive(FromQueryResult, attributes(turso))]
pub fn derive_from_query_result(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    from_query_result::expand(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derives `ActiveEnum` and the column-type impls for a fieldless enum.
///
/// Struct attribute: `#[turso(rs_type = "String")]` or an integer type such
/// as `"i32"`. Variant attributes: `#[turso(string_value = "...")]`, which
/// defaults to the `snake_case` variant name, or `#[turso(num_value = 1)]`,
/// which is required for integer-backed enums.
#[proc_macro_derive(DeriveActiveEnum, attributes(turso))]
pub fn derive_active_enum(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    active_enum::expand(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derives `PartialModelTrait` and `FromQueryResult` for a projection struct.
///
/// Struct attribute: `#[turso(entity = "path::Entity")]`, required. Field
/// attributes: `#[turso(from_col = "Variant")]` to read another column than
/// the one named after the field, `#[turso(from_expr = "...")]` to read an
/// expression written in Rust against `Expr`.
#[proc_macro_derive(DerivePartialModel, attributes(turso))]
pub fn derive_partial_model(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    partial_model::expand(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derives `IntoActiveModel` for a plain struct whose fields are a subset
/// of an entity's columns.
///
/// Struct attribute: `#[turso(active_model = "path::ActiveModel")]`, the
/// `ActiveModel` in scope by default. Field attribute: `#[turso(ignore)]`.
/// A plain field becomes `Set`; an `Option` around the attribute type
/// becomes `Set` when `Some` and `NotSet` when `None`.
#[proc_macro_derive(DeriveIntoActiveModel, attributes(turso))]
pub fn derive_into_active_model(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    into_active_model::expand(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derives `MigrationName` from the module path.
///
/// The name is the last segment of `module_path!()` evaluated where the
/// derive is written, so a migration in module
/// `m20240101_000001_create_user` is named exactly that. Deriving it from
/// the module rather than the type keeps every migration struct free to be
/// called `Migration`.
#[proc_macro_derive(DeriveMigrationName)]
pub fn derive_migration_name(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    let ident = input.ident;
    quote::quote! {
        impl ::turso_orm_migration::MigrationName for #ident {
            fn name(&self) -> &str {
                let path = module_path!();
                path.rsplit("::").next().unwrap_or(path)
            }
        }
    }
    .into()
}
