//! Relations between entities, modeled by [`RelationDef`], [`Related`] and [`Linked`].
//!
//! A relation is a static description of how two tables join: the columns
//! on each side, which side owns the foreign key, and the referential
//! actions. The same definition serves three consumers — the `JOIN ... ON`
//! condition of a select, the batch loaders, and the `FOREIGN KEY` clause
//! that [`Schema`](super::Schema) emits — so it is kept as plain data rather
//! than behaviour.
//!
//! `belongs_to` and `has_one` share [`RelationType::HasOne`]; what tells
//! them apart is [`RelationDef::is_owner`], which records whether the
//! declaring table holds the foreign key and therefore whether a foreign
//! key constraint is generated for it.
//!
//! A many-to-many relation is two hops through a junction table:
//! [`Related::via`] names the first hop and [`Related::to`] the second. A
//! [`Linked`] chain generalises that to any number of hops, each one an
//! ordinary [`RelationDef`], so that a query can follow a path of relations
//! without the entities in between having to be loaded.
//!
//! Join conditions can be rendered against table aliases through
//! [`RelationDef::join_condition_refs`], which is what makes self-referencing
//! relations and chains that revisit a table unambiguous.

use turso_sql::{Expr, ForeignKey, ForeignKeyAction, JoinType};

use super::base_entity::EntityTrait;
use super::column::ColumnTrait;
use super::iden::{IdenStatic, Iterable};

/// The cardinality of a relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationType {
    /// At most one row of the target.
    HasOne,
    /// Any number of rows of the target.
    HasMany,
}

/// A relation from one entity (`from`) to another (`to`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelationDef {
    /// The cardinality.
    pub rel_type: RelationType,
    /// The table of the entity that declares the relation.
    pub from_tbl: &'static str,
    /// The table being related to.
    pub to_tbl: &'static str,
    /// The join columns on `from_tbl`, paired by position with `to_col`.
    pub from_col: Vec<&'static str>,
    /// The join columns on `to_tbl`, paired by position with `from_col`.
    pub to_col: Vec<&'static str>,
    /// Whether `from_tbl` holds the foreign key, as in `belongs_to`.
    pub is_owner: bool,
    /// The `ON DELETE` action for the generated foreign key.
    pub on_delete: Option<ForeignKeyAction>,
    /// The `ON UPDATE` action for the generated foreign key.
    pub on_update: Option<ForeignKeyAction>,
    /// Whether to skip generating a foreign key for this relation.
    pub skip_fk: bool,
}

impl RelationDef {
    /// The reverse relation, from `to` back to `from`.
    ///
    /// Ownership flips with the direction so that the foreign key is still
    /// attributed to the table that holds it.
    #[must_use]
    pub fn rev(self) -> Self {
        Self {
            rel_type: self.rel_type,
            from_tbl: self.to_tbl,
            to_tbl: self.from_tbl,
            from_col: self.to_col,
            to_col: self.from_col,
            is_owner: !self.is_owner,
            on_delete: self.on_delete,
            on_update: self.on_update,
            skip_fk: self.skip_fk,
        }
    }

    /// Builds the join condition `from.a = to.a AND from.b = to.b`.
    pub fn join_condition(&self) -> Expr {
        self.join_condition_refs(self.from_tbl, self.to_tbl)
    }

    /// Builds the join condition with each side qualified by the given
    /// reference — a table name or the alias it was joined under.
    ///
    /// Once a table is aliased, SQL requires every qualified column to use
    /// the alias, so the select builder passes the reference it chose for
    /// each side instead of the table names stored on the relation.
    pub fn join_condition_refs(&self, from_ref: &str, to_ref: &str) -> Expr {
        let mut cond: Option<Expr> = None;
        for (f, t) in self.from_col.iter().zip(&self.to_col) {
            let e = Expr::col((from_ref.to_owned(), *f)).eq(Expr::col((to_ref.to_owned(), *t)));
            cond = Some(match cond {
                Some(c) => c.and(e),
                None => e,
            });
        }
        // A relation without columns cannot restrict the join; `TRUE` keeps
        // the rendered SQL valid rather than emitting an empty `ON`.
        cond.unwrap_or_else(|| Expr::val(true))
    }

    /// The foreign key this relation implies, when the declaring table owns
    /// it and generation was not skipped.
    pub fn foreign_key(&self) -> Option<ForeignKey> {
        if !self.is_owner || self.skip_fk {
            return None;
        }
        let mut fk = ForeignKey::new(
            self.from_col.iter().copied(),
            self.to_tbl,
            self.to_col.iter().copied(),
        );
        if let Some(a) = self.on_delete {
            fk = fk.on_delete(a);
        }
        if let Some(a) = self.on_update {
            fk = fk.on_update(a);
        }
        Some(fk)
    }

    /// The join type `find_also_related` uses: `LEFT`, so that rows without a
    /// related row are still returned.
    pub fn default_join(&self) -> JoinType {
        JoinType::Left
    }
}

/// The relations an entity declares, derived on the `Relation` enum.
pub trait RelationTrait: Iterable + std::fmt::Debug {
    /// The definition of this relation.
    fn def(&self) -> RelationDef;
}

/// Declares that `Self` is related to `R`, directly or through a junction.
///
/// For a direct relation only [`to`](Self::to) is defined. For a
/// many-to-many relation, [`via`](Self::via) is the hop from `Self` to the
/// junction table and [`to`](Self::to) the hop from the junction to `R`;
/// every consumer — `find_related`, the joins, `find_also_related` and the
/// loaders — follows both hops when `via` is present.
pub trait Related<R: EntityTrait>: EntityTrait {
    /// The relation reaching `R`: from `Self` directly, or from the junction
    /// table when [`via`](Self::via) is defined.
    fn to() -> RelationDef;

    /// The relation from `Self` to the junction table of a many-to-many
    /// relation, or `None` for a direct relation.
    fn via() -> Option<RelationDef> {
        None
    }
}

/// A path of relations from one entity to another, through any number of
/// intermediate tables.
///
/// Each hop is a [`RelationDef`] whose `from_tbl` is the `to_tbl` of the
/// previous one, starting at [`FromEntity`](Self::FromEntity) and ending at
/// [`ToEntity`](Self::ToEntity). A chain is written by hand as a unit
/// struct, which keeps multi-hop paths explicit and lets the same two
/// entities be linked by several different paths:
///
/// ```ignore
/// pub struct PostToTag;
///
/// impl Linked for PostToTag {
///     type FromEntity = post::Entity;
///     type ToEntity = tag::Entity;
///
///     fn link(&self) -> Vec<RelationDef> {
///         vec![
///             post_tag::Relation::Post.def().rev(),
///             post_tag::Relation::Tag.def(),
///         ]
///     }
/// }
/// ```
pub trait Linked {
    /// The entity the chain starts from.
    type FromEntity: EntityTrait;
    /// The entity the chain ends at.
    type ToEntity: EntityTrait;

    /// The hops, in order from [`FromEntity`](Self::FromEntity) to
    /// [`ToEntity`](Self::ToEntity).
    fn link(&self) -> Vec<RelationDef>;
}

/// Looks up a column variant of `E` by its SQL name.
///
/// Relation definitions store column names as strings, so consumers that
/// need to read a model attribute have to map them back to variants.
pub(crate) fn column_of<E: EntityTrait>(name: &str) -> Option<E::Column> {
    E::Column::iter().find(|c| c.as_str() == name)
}

/// A builder for a [`RelationDef`], returned by [`EntityTrait::belongs_to`],
/// [`EntityTrait::has_one`] and [`EntityTrait::has_many`].
#[derive(Debug)]
pub struct RelationBuilder<E: EntityTrait, R: EntityTrait> {
    /// The definition being assembled.
    def: RelationDef,
    /// Ties the builder to the two entity types without storing them.
    _e: std::marker::PhantomData<(E, R)>,
}

impl<E: EntityTrait, R: EntityTrait> RelationBuilder<E, R> {
    /// Starts a relation of the given cardinality and ownership between `E` and `R`.
    pub(crate) fn new(rel_type: RelationType, is_owner: bool) -> Self {
        Self {
            def: RelationDef {
                rel_type,
                from_tbl: E::TABLE_NAME,
                to_tbl: R::TABLE_NAME,
                from_col: Vec::new(),
                to_col: Vec::new(),
                is_owner,
                on_delete: None,
                on_update: None,
                skip_fk: false,
            },
            _e: std::marker::PhantomData,
        }
    }

    /// Adds a join column on `E`; call once per column of a composite key.
    #[must_use]
    pub fn from<C: ColumnTrait>(mut self, column: C) -> Self {
        self.def.from_col.push(column.as_str());
        self
    }

    /// Adds a join column on `R`; call once per column of a composite key.
    #[must_use]
    pub fn to<C: ColumnTrait>(mut self, column: C) -> Self {
        self.def.to_col.push(column.as_str());
        self
    }

    /// Sets the `ON DELETE` action of the generated foreign key.
    #[must_use]
    pub fn on_delete(mut self, action: ForeignKeyAction) -> Self {
        self.def.on_delete = Some(action);
        self
    }

    /// Sets the `ON UPDATE` action of the generated foreign key.
    #[must_use]
    pub fn on_update(mut self, action: ForeignKeyAction) -> Self {
        self.def.on_update = Some(action);
        self
    }

    /// Disables foreign key generation for this relation.
    #[must_use]
    pub fn skip_fk(mut self) -> Self {
        self.def.skip_fk = true;
        self
    }
}

impl<E: EntityTrait, R: EntityTrait> From<RelationBuilder<E, R>> for RelationDef {
    fn from(b: RelationBuilder<E, R>) -> Self {
        b.def
    }
}
