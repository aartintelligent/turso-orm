//! Batch loading of related rows, modeled by [`LoaderTrait`].
//!
//! Given a list of models, the loaders fetch every related row in a single
//! query and hand them back aligned with the input, which avoids the N+1
//! pattern of calling `find_related` in a loop. The trait is implemented on
//! `Vec<M>` and `[M]` so that the result of `.all(db)` can be passed
//! straight in.
//!
//! Rows are grouped by the literal rendering of their key values rather
//! than by the values themselves. That sidesteps the need for `Hash` and
//! `Eq` on every key type — `f64` has neither — at the cost of a small
//! string per row, and the literal form is unambiguous because
//! `Value::to_literal` quotes text and distinguishes `1` from `1.0`.
//!
//! A many-to-many relation is loaded through its junction table in one
//! query too: the related rows are selected together with the junction
//! columns that point back at the input models, aliased `__via_<col>`, and
//! grouped on those.

use std::collections::HashMap;

use async_trait::async_trait;
use turso_orm_driver::{ConnectionTrait, Row};
use turso_sql::{Condition, Expr, JoinType, Value};

use super::base_entity::EntityTrait;
use super::column::ColumnTrait;
use super::model::{FromQueryResult, ModelTrait};
use super::relation::{Related, RelationDef, column_of};
use crate::{DbErr, Result};

/// Loads related rows for a batch of models in one query.
#[async_trait]
pub trait LoaderTrait {
    /// The entity of the models in the batch.
    type Entity: EntityTrait;

    /// Loads, for each model, the related `R` row if any; for `has_one` and `belongs_to`.
    ///
    /// The result has one entry per input model, in input order.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Custom`] when the relation names a column the
    /// entity does not have; [`DbErr::Driver`] when the query fails or a
    /// row cannot be decoded.
    async fn load_one<R, C>(&self, _: R, db: &C) -> Result<Vec<Option<R::Model>>>
    where
        R: EntityTrait,
        Self::Entity: Related<R>,
        C: ConnectionTrait;

    /// Loads, for each model, the related `R` rows; for `has_many`, and for
    /// a many-to-many relation, which is followed through its junction.
    ///
    /// The result has one entry per input model, in input order.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Custom`] when the relation names a column the
    /// entity does not have; [`DbErr::Driver`] when the query fails or a
    /// row cannot be decoded.
    async fn load_many<R, C>(&self, _: R, db: &C) -> Result<Vec<Vec<R::Model>>>
    where
        R: EntityTrait,
        Self::Entity: Related<R>,
        C: ConnectionTrait;

    /// Loads, for each model, the `R` rows reached through the junction
    /// table of a many-to-many relation.
    ///
    /// [`load_many`](Self::load_many) does the same when the relation
    /// declares a junction; this method exists to make the intent explicit
    /// and fails on a direct relation.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Custom`] when the relation has no junction or names
    /// a column the entity does not have; [`DbErr::Driver`] when the query
    /// fails or a row cannot be decoded.
    async fn load_many_to_many<R, C>(&self, _: R, db: &C) -> Result<Vec<Vec<R::Model>>>
    where
        R: EntityTrait,
        Self::Entity: Related<R>,
        C: ConnectionTrait;
}

/// Renders key values as a grouping key.
///
/// The unit separator keeps composite keys unambiguous even when a text
/// component could itself contain the literal of another component.
fn key(values: &[Value]) -> String {
    values
        .iter()
        .map(Value::to_literal)
        .collect::<Vec<_>>()
        .join("\u{1f}")
}

/// Maps the column names of one side of a relation to column variants of `E`.
///
/// # Errors
///
/// Returns [`DbErr::Custom`] when a name is not a column of `E`.
fn columns<E: EntityTrait>(names: &[&'static str]) -> Result<Vec<E::Column>> {
    names
        .iter()
        .map(|n| column_of::<E>(n).ok_or_else(|| DbErr::Custom(format!("unknown column {n}"))))
        .collect()
}

/// The grouping key of every model, from its `from_cols`, in input order.
fn keys_of<E: EntityTrait>(models: &[E::Model], from_cols: &[E::Column]) -> Vec<String> {
    models
        .iter()
        .map(|m| key(&from_cols.iter().map(|c| m.get(*c)).collect::<Vec<_>>()))
        .collect()
}

/// Builds the condition matching the keys of `models` on `targets`, which
/// are column expressions on the related or junction side.
///
/// A single key column renders `IN (...)`; SQLite has no row-value `IN`,
/// so composite keys become an `OR` of one `AND` group per model.
fn keys_condition<E: EntityTrait>(
    models: &[E::Model],
    from_cols: &[E::Column],
    targets: &[Expr],
) -> Condition {
    if targets.len() == 1 {
        let values: Vec<Value> = models.iter().map(|m| m.get(from_cols[0])).collect();
        return Condition::all().add(targets[0].clone().is_in(values));
    }
    let mut cond = Condition::any();
    for m in models {
        let mut c = Condition::all();
        for (f, t) in from_cols.iter().zip(targets) {
            c = c.add(t.clone().eq(Expr::val(m.get(*f))));
        }
        cond = cond.add(c);
    }
    cond
}

/// Fetches every `R` row directly related to `models` and groups them by key.
///
/// Returns the grouping key of each input model, in order, alongside the
/// grouped rows, so that the callers can align the two without re-deriving
/// keys.
///
/// # Errors
///
/// Returns [`DbErr::Custom`] when the relation names an unknown column;
/// [`DbErr::Driver`] when the query fails or a row cannot be decoded.
async fn load_direct<E, R, C>(
    def: &RelationDef,
    models: &[E::Model],
    db: &C,
) -> Result<(Vec<String>, HashMap<String, Vec<R::Model>>)>
where
    E: EntityTrait,
    R: EntityTrait,
    C: ConnectionTrait,
{
    let from_cols = columns::<E>(&def.from_col)?;
    let to_cols = columns::<R>(&def.to_col)?;
    let keys = keys_of::<E>(models, &from_cols);
    let mut grouped: HashMap<String, Vec<R::Model>> = HashMap::new();
    // An empty batch would render `IN ()`, which is not valid SQL.
    if models.is_empty() {
        return Ok((keys, grouped));
    }
    let targets: Vec<Expr> = to_cols.iter().map(|c| c.into_expr()).collect();
    let query = R::find().filter(keys_condition::<E>(models, &from_cols, &targets));
    for related in query.all(db).await? {
        let k = key(&to_cols.iter().map(|c| related.get(*c)).collect::<Vec<_>>());
        grouped.entry(k).or_default().push(related);
    }
    Ok((keys, grouped))
}

/// Fetches every `R` row related to `models` through a junction table and
/// groups them by the junction columns pointing back at the models.
///
/// # Errors
///
/// Returns [`DbErr::Custom`] when a relation names an unknown column;
/// [`DbErr::Driver`] when the query fails or a row cannot be decoded.
async fn load_via<E, R, C>(
    via: &RelationDef,
    to: &RelationDef,
    models: &[E::Model],
    db: &C,
) -> Result<(Vec<String>, HashMap<String, Vec<R::Model>>)>
where
    E: EntityTrait,
    R: EntityTrait,
    C: ConnectionTrait,
{
    let from_cols = columns::<E>(&via.from_col)?;
    let keys = keys_of::<E>(models, &from_cols);
    let mut grouped: HashMap<String, Vec<R::Model>> = HashMap::new();
    if models.is_empty() {
        return Ok((keys, grouped));
    }
    let mut query = R::find();
    let junction = query.join_table(JoinType::Inner, to.from_tbl, |r| {
        to.join_condition_refs(r, R::TABLE_NAME)
    });
    let targets: Vec<Expr> = via
        .to_col
        .iter()
        .map(|c| Expr::col((junction.clone(), *c)))
        .collect();
    // The junction columns ride along under a prefix no entity column can
    // carry, so that the related model still decodes from the same row.
    let aliases: Vec<String> = via.to_col.iter().map(|c| format!("__via_{c}")).collect();
    for (target, alias) in targets.iter().zip(&aliases) {
        query = query.expr_as(target.clone(), alias.clone());
    }
    let query = query.filter(keys_condition::<E>(models, &from_cols, &targets));
    for row in query.into_model::<Row>().all(db).await? {
        let related = R::Model::from_query_result(&row, "")?;
        let values: Vec<Value> = aliases
            .iter()
            .map(|a| row.raw(a.as_str()).cloned().unwrap_or(Value::Null))
            .collect();
        grouped.entry(key(&values)).or_default().push(related);
    }
    Ok((keys, grouped))
}

#[async_trait]
impl<M: ModelTrait> LoaderTrait for Vec<M> {
    type Entity = M::Entity;

    async fn load_one<R, C>(&self, r: R, db: &C) -> Result<Vec<Option<R::Model>>>
    where
        R: EntityTrait,
        Self::Entity: Related<R>,
        C: ConnectionTrait,
    {
        self.as_slice().load_one(r, db).await
    }

    async fn load_many<R, C>(&self, r: R, db: &C) -> Result<Vec<Vec<R::Model>>>
    where
        R: EntityTrait,
        Self::Entity: Related<R>,
        C: ConnectionTrait,
    {
        self.as_slice().load_many(r, db).await
    }

    async fn load_many_to_many<R, C>(&self, r: R, db: &C) -> Result<Vec<Vec<R::Model>>>
    where
        R: EntityTrait,
        Self::Entity: Related<R>,
        C: ConnectionTrait,
    {
        self.as_slice().load_many_to_many(r, db).await
    }
}

#[async_trait]
impl<M: ModelTrait> LoaderTrait for [M] {
    type Entity = M::Entity;

    async fn load_one<R, C>(&self, _: R, db: &C) -> Result<Vec<Option<R::Model>>>
    where
        R: EntityTrait,
        Self::Entity: Related<R>,
        C: ConnectionTrait,
    {
        let def = <M::Entity as Related<R>>::to();
        let (keys, mut grouped) = load_direct::<M::Entity, R, C>(&def, self, db).await?;
        // Several input models may share a key, so the first match is cloned
        // rather than moved out of the group.
        Ok(keys
            .iter()
            .map(|k| {
                grouped.get_mut(k).and_then(|v| {
                    if v.is_empty() {
                        None
                    } else {
                        Some(v[0].clone())
                    }
                })
            })
            .collect())
    }

    async fn load_many<R, C>(&self, r: R, db: &C) -> Result<Vec<Vec<R::Model>>>
    where
        R: EntityTrait,
        Self::Entity: Related<R>,
        C: ConnectionTrait,
    {
        if <M::Entity as Related<R>>::via().is_some() {
            return self.load_many_to_many(r, db).await;
        }
        let def = <M::Entity as Related<R>>::to();
        let (keys, grouped) = load_direct::<M::Entity, R, C>(&def, self, db).await?;
        Ok(keys
            .iter()
            .map(|k| grouped.get(k).cloned().unwrap_or_default())
            .collect())
    }

    async fn load_many_to_many<R, C>(&self, _: R, db: &C) -> Result<Vec<Vec<R::Model>>>
    where
        R: EntityTrait,
        Self::Entity: Related<R>,
        C: ConnectionTrait,
    {
        let via = <M::Entity as Related<R>>::via().ok_or_else(|| {
            DbErr::Custom(format!(
                "{} is not related to {} through a junction table",
                M::Entity::TABLE_NAME,
                R::TABLE_NAME
            ))
        })?;
        let to = <M::Entity as Related<R>>::to();
        let (keys, grouped) = load_via::<M::Entity, R, C>(&via, &to, self, db).await?;
        Ok(keys
            .iter()
            .map(|k| grouped.get(k).cloned().unwrap_or_default())
            .collect())
    }
}
