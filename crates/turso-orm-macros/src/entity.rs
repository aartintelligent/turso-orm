//! The `DeriveEntityModel` expansion.
//!
//! From one `Model` struct the derive generates the whole entity module
//! surface: the `Entity` unit struct, the `Column` and `PrimaryKey` enums,
//! the `ActiveModel` struct, and the trait impls that tie them to
//! `turso_orm::entity`. The user still writes the `Relation` enum and the
//! `ActiveModelBehavior` impl by hand, because both carry decisions the
//! derive cannot infer.
//!
//! Column types, nullability and auto-increment eligibility are not decided
//! here; the generated code reads them from the `TursoType` impl of each
//! field type, so adding a supported type never touches the macro. The
//! generated `Column` enum deliberately does not derive `PartialEq`, so that
//! `Column::X.eq(v)` resolves to the `ColumnTrait` condition builder.
//!
//! A field marked `ignore` is not a column: it is left out of every
//! generated enum and of the active model, and the decoder fills it with
//! `Default::default()`, so its type must implement `Default`.

use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Expr, Fields, Type};

use crate::attrs;
use crate::iden::impl_iden;

/// One model field after attribute parsing.
struct Field {
    /// The field identifier.
    ident: syn::Ident,
    /// The `UpperCamelCase` variant name used in `Column` and `PrimaryKey`.
    variant: syn::Ident,
    /// The field type.
    ty: Type,
    /// The SQL column name; the field name unless overridden.
    column_name: String,
    /// Whether the field is part of the primary key.
    primary_key: bool,
    /// The explicit `auto_increment` setting, or `None` to infer it from the type.
    auto_increment: Option<bool>,
    /// Whether the column is `UNIQUE`.
    unique: bool,
    /// Whether a secondary index is generated for the column.
    indexed: bool,
    /// The `default_value` expression, if any.
    default_value: Option<Expr>,
}

/// Whether `ty` is spelled `Option<..>`.
///
/// Kept for callers that want to inspect the type syntactically; the
/// generated code reads nullability from `TursoType::NULLABLE` instead.
fn is_option(ty: &Type) -> bool {
    if let Type::Path(p) = ty
        && let Some(seg) = p.path.segments.last()
    {
        return seg.ident == "Option";
    }
    false
}

/// Expands a `Model` struct into the entity module surface.
///
/// # Errors
///
/// Returns a compile error when the input is not a struct with named
/// fields, when the struct attribute is missing `table_name` or carries an
/// unknown key, when a field carries an unknown key, when an attribute value
/// has the wrong literal type, or when no field is marked `primary_key`.
pub(crate) fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let model = &input.ident;
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            model,
            "DeriveEntityModel only supports structs",
        ));
    };
    let Fields::Named(named) = &data.fields else {
        return Err(syn::Error::new_spanned(
            model,
            "DeriveEntityModel needs named fields",
        ));
    };

    let mut table_name = None;
    for item in attrs::parse(&input.attrs)? {
        match item.key.as_str() {
            "table_name" => table_name = Some(item.str_value()?),
            other => {
                return Err(syn::Error::new(
                    item.span,
                    format!("unknown entity attribute `{other}`"),
                ));
            }
        }
    }
    let table_name = table_name.ok_or_else(|| {
        syn::Error::new_spanned(model, "missing `#[turso(table_name = \"...\")]`")
    })?;

    let mut fields = Vec::new();
    let mut ignored: Vec<syn::Ident> = Vec::new();
    for f in &named.named {
        // Unnamed fields were rejected above, so this `expect` is unreachable.
        let ident = f.ident.clone().expect("named");
        let mut field = Field {
            variant: format_ident!("{}", ident.to_string().to_upper_camel_case()),
            column_name: ident.to_string(),
            ident,
            ty: f.ty.clone(),
            primary_key: false,
            auto_increment: None,
            unique: false,
            indexed: false,
            default_value: None,
        };
        let mut ignore = false;
        for item in attrs::parse(&f.attrs)? {
            match item.key.as_str() {
                "primary_key" => field.primary_key = item.bool_value()?,
                "auto_increment" => field.auto_increment = Some(item.bool_value()?),
                "column_name" => field.column_name = item.str_value()?,
                "unique" => field.unique = item.bool_value()?,
                "indexed" => field.indexed = item.bool_value()?,
                // Nullability is implied by `Option<T>`; the key is accepted
                // and ignored so that explicit declarations do not fail.
                "nullable" => {}
                "default_value" => field.default_value.clone_from(&item.value),
                "ignore" => ignore = item.bool_value()?,
                other => {
                    return Err(syn::Error::new(
                        item.span,
                        format!("unknown column attribute `{other}`"),
                    ));
                }
            }
        }
        if ignore {
            ignored.push(field.ident);
        } else {
            fields.push(field);
        }
    }

    let pks: Vec<&Field> = fields.iter().filter(|f| f.primary_key).collect();
    if pks.is_empty() {
        return Err(syn::Error::new_spanned(
            model,
            "an entity needs at least one `#[turso(primary_key)]` field",
        ));
    }

    let col_variants: Vec<&syn::Ident> = fields.iter().map(|f| &f.variant).collect();
    let col_names: Vec<&String> = fields.iter().map(|f| &f.column_name).collect();
    let pk_variants: Vec<&syn::Ident> = pks.iter().map(|f| &f.variant).collect();

    // Column definitions read type, nullability and constraints through the
    // field's `TursoType` impl so the macro never has to know the type.
    let col_defs = fields.iter().map(|f| {
        let v = &f.variant;
        let ty = &f.ty;
        let unique = f.unique.then(|| quote! { .unique() });
        let indexed = f.indexed.then(|| quote! { .indexed() });
        let default = f.default_value.as_ref().map(|d| quote! { .default(#d) });
        quote! {
            Self::#v => ::turso_orm::entity::ColumnDef::new(<#ty as ::turso_orm::types::TursoType>::COLUMN_TYPE)
                .nullable(<#ty as ::turso_orm::types::TursoType>::NULLABLE)
                #unique #indexed #default,
        }
    });

    // A single key uses the field type directly; a composite key is a tuple
    // in declaration order. Only a single integral key can auto-increment,
    // unless the user overrode it explicitly.
    let pk_value_type = if pks.len() == 1 {
        let t = &pks[0].ty;
        quote! { #t }
    } else {
        let ts = pks.iter().map(|f| &f.ty);
        quote! { (#(#ts),*) }
    };
    let auto_increment = if pks.len() == 1 {
        let t = &pks[0].ty;
        if let Some(b) = pks[0].auto_increment {
            quote! { #b }
        } else {
            quote! { <#t as ::turso_orm::types::TursoType>::INTEGRAL }
        }
    } else {
        quote! { false }
    };
    let pk_to_column = pks.iter().map(|f| {
        let v = &f.variant;
        quote! { Self::#v => Column::#v, }
    });
    let column_to_pk = pks.iter().map(|f| {
        let v = &f.variant;
        quote! { Column::#v => Some(Self::#v), }
    });

    // Model accessors: `get` converts through `Into<Value>`, `set` decodes
    // through `decode_field` and reports a bad value as an error.
    let model_get = fields.iter().map(|f| {
        let v = &f.variant;
        let i = &f.ident;
        quote! { Column::#v => ::core::convert::Into::<::turso_orm::Value>::into(self.#i.clone()), }
    });
    let model_set = fields.iter().map(|f| {
        let v = &f.variant;
        let i = &f.ident;
        let ty = &f.ty;
        let name = &f.column_name;
        quote! { Column::#v => { self.#i = ::turso_orm::__private::decode_field::<#ty>(#name, value)?; } }
    });
    let from_row = fields.iter().map(|f| {
        let i = &f.ident;
        let ty = &f.ty;
        let name = &f.column_name;
        quote! { #i: ::turso_orm::__private::get_field::<#ty>(row, prefix, #name)?, }
    });
    // Ignored fields have no column to read, so both the row decoder and the
    // active-model conversion fill them with their default.
    let ignored_defaults = ignored
        .iter()
        .map(|i| quote! { #i: ::core::default::Default::default(), })
        .collect::<Vec<_>>();
    let into_model = fields.iter().map(|f| {
        let i = &f.ident;
        let name = &f.column_name;
        quote! {
            #i: self.#i.into_value().ok_or_else(|| ::turso_orm::DbErr::AttrNotSet(#name.into()))?,
        }
    });

    // Active model: one `ActiveValue<T>` per field plus the per-column
    // state accessors the trait requires.
    let am_fields = fields.iter().map(|f| {
        let i = &f.ident;
        let ty = &f.ty;
        quote! { pub #i: ::turso_orm::entity::ActiveValue<#ty>, }
    });
    let am_get = fields.iter().map(|f| {
        let v = &f.variant;
        let i = &f.ident;
        quote! { Column::#v => self.#i.clone().map(::core::convert::Into::into), }
    });
    let am_set = fields.iter().map(|f| {
        let v = &f.variant;
        let i = &f.ident;
        let ty = &f.ty;
        let name = &f.column_name;
        quote! { Column::#v => { self.#i = ::turso_orm::entity::ActiveValue::Set(::turso_orm::__private::decode_field::<#ty>(#name, value)?); } }
    });
    let am_not_set = fields.iter().map(|f| {
        let v = &f.variant;
        let i = &f.ident;
        quote! { Column::#v => { self.#i = ::turso_orm::entity::ActiveValue::NotSet; } }
    });
    let am_is_not_set = fields.iter().map(|f| {
        let v = &f.variant;
        let i = &f.ident;
        quote! { Column::#v => self.#i.is_not_set(), }
    });
    let am_reset = fields.iter().map(|f| {
        let v = &f.variant;
        let i = &f.ident;
        quote! { Column::#v => self.#i.reset(), }
    });
    let am_from_model = fields.iter().map(|f| {
        let i = &f.ident;
        quote! { #i: ::turso_orm::entity::ActiveValue::Unchanged(m.#i), }
    });
    let am_default = fields.iter().map(|f| {
        let i = &f.ident;
        quote! { #i: ::turso_orm::entity::ActiveValue::NotSet, }
    });

    let entity_iden = impl_iden(&format_ident!("Entity"), quote! { #table_name });
    let column_iden = impl_iden(
        &format_ident!("Column"),
        quote! { match self { #(Self::#col_variants => #col_names,)* } },
    );
    let pk_iden = impl_iden(
        &format_ident!("PrimaryKey"),
        quote! { match self { #(Self::#pk_variants => <Column as ::turso_orm::entity::IdenStatic>::as_str(&Column::#pk_variants),)* } },
    );

    let vis = &input.vis;
    // Nullability comes from `TursoType::NULLABLE`, so the syntactic check
    // is not needed here; the binding keeps the helper available without a
    // dead-code warning.
    let _ = is_option;

    // `from_column` carries `#[allow(unreachable_patterns)]` because when
    // every column is part of the key the trailing `_ => None` arm can never
    // match, and the derive cannot know that in advance.
    Ok(quote! {
        /// The entity.
        #[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
        #vis struct Entity;

        #entity_iden

        impl ::turso_orm::entity::EntityTrait for Entity {
            type Model = #model;
            type Column = Column;
            type PrimaryKey = PrimaryKey;
            type ActiveModel = ActiveModel;
            type Relation = Relation;
            const TABLE_NAME: &'static str = #table_name;
        }

        /// The columns of the entity.
        ///
        /// Deliberately not `PartialEq`: `Column::Name.eq(..)` must resolve to
        /// the condition builder.
        #[derive(Copy, Clone, Debug)]
        #vis enum Column {
            #(#col_variants,)*
        }

        #column_iden

        impl ::turso_orm::entity::Iterable for Column {
            const ALL: &'static [Self] = &[#(Self::#col_variants),*];
        }

        impl ::turso_orm::entity::ColumnTrait for Column {
            const TABLE: &'static str = #table_name;

            fn def(&self) -> ::turso_orm::entity::ColumnDef {
                match self { #(#col_defs)* }
            }
        }

        /// The primary key of the entity.
        #[derive(Copy, Clone, Debug)]
        #vis enum PrimaryKey {
            #(#pk_variants,)*
        }

        #pk_iden

        impl ::turso_orm::entity::Iterable for PrimaryKey {
            const ALL: &'static [Self] = &[#(Self::#pk_variants),*];
        }

        impl ::turso_orm::entity::PrimaryKeyTrait for PrimaryKey {
            type ValueType = #pk_value_type;

            fn auto_increment() -> bool {
                #auto_increment
            }
        }

        impl ::turso_orm::entity::PrimaryKeyToColumn for PrimaryKey {
            type Column = Column;

            fn into_column(self) -> Column {
                match self { #(#pk_to_column)* }
            }

            fn from_column(column: Column) -> Option<Self> {
                #[allow(unreachable_patterns)]
                match column { #(#column_to_pk)* _ => None }
            }
        }

        impl ::turso_orm::entity::ModelTrait for #model {
            type Entity = Entity;

            fn get(&self, column: Column) -> ::turso_orm::Value {
                match column { #(#model_get)* }
            }

            fn set(&mut self, column: Column, value: ::turso_orm::Value) -> ::turso_orm::Result<()> {
                match column { #(#model_set)* }
                Ok(())
            }
        }

        impl ::turso_orm::entity::FromQueryResult for #model {
            fn from_query_result(row: &::turso_orm::__private::Row, prefix: &str) -> ::turso_orm::Result<Self> {
                Ok(Self { #(#from_row)* #(#ignored_defaults)* })
            }
        }

        /// The mutable form of the model.
        #[derive(Clone, Debug, PartialEq)]
        #vis struct ActiveModel {
            #(#am_fields)*
        }

        impl ::core::default::Default for ActiveModel {
            fn default() -> Self {
                Self { #(#am_default)* }
            }
        }

        impl ::core::convert::From<#model> for ActiveModel {
            fn from(m: #model) -> Self {
                Self { #(#am_from_model)* }
            }
        }

        impl ::turso_orm::entity::IntoActiveModel<ActiveModel> for #model {
            fn into_active_model(self) -> ActiveModel {
                self.into()
            }
        }

        impl ::turso_orm::entity::TryIntoModel<#model> for ActiveModel {
            fn try_into_model(self) -> ::turso_orm::Result<#model> {
                Ok(#model { #(#into_model)* #(#ignored_defaults)* })
            }
        }

        #[::turso_orm::__private::async_trait]
        impl ::turso_orm::entity::ActiveModelTrait for ActiveModel {
            type Entity = Entity;

            fn get(&self, column: Column) -> ::turso_orm::entity::ActiveValue<::turso_orm::Value> {
                match column { #(#am_get)* }
            }

            fn set(&mut self, column: Column, value: ::turso_orm::Value) -> ::turso_orm::Result<()> {
                match column { #(#am_set)* }
                Ok(())
            }

            fn not_set(&mut self, column: Column) {
                match column { #(#am_not_set)* }
            }

            fn is_not_set(&self, column: Column) -> bool {
                match column { #(#am_is_not_set)* }
            }

            fn reset(&mut self, column: Column) {
                match column { #(#am_reset)* }
            }
        }
    })
}
