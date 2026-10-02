//! Schema generation from entities, modeled by [`Schema`].
//!
//! The generator reads the static column, key and relation definitions of an
//! entity and produces `turso_sql` DDL builders; it never executes anything,
//! so callers decide whether to run the statements directly or hand them to
//! a migration. Column defaults are inlined as literals by the SQL layer
//! because SQLite does not bind parameters in DDL.
//!
//! A single-column primary key is declared inline on the column so that an
//! integral key becomes a true `INTEGER PRIMARY KEY` row id alias; composite
//! keys fall back to a table-level `PRIMARY KEY (...)` clause.

use turso_sql::{ColumnDef as SqlColumnDef, CreateIndex, CreateTable, Table};

use super::base_entity::EntityTrait;
use super::column::ColumnTrait;
use super::iden::{IdenStatic, Iterable};
use super::primary_key::{PrimaryKeyToColumn, PrimaryKeyTrait};
use super::relation::RelationTrait;

/// A generator of DDL statements from entity definitions.
#[derive(Debug, Default)]
pub struct Schema {
    /// Whether generated tables carry the `STRICT` table option.
    strict: bool,
}

impl Schema {
    /// A schema generator with the default, non-strict settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether generated tables are declared `STRICT`.
    #[must_use]
    pub fn strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    /// Builds the `CREATE TABLE` statement for an entity, including the
    /// foreign keys implied by its `belongs_to` relations.
    pub fn create_table_from_entity<E: EntityTrait>(&self, _: E) -> CreateTable {
        let mut create = Table::create().table(E::TABLE_NAME);
        let pks: Vec<E::PrimaryKey> = E::PrimaryKey::iter().collect();
        let single_pk = pks.len() == 1;
        for column in E::Column::iter() {
            let def = column.def();
            let mut col = SqlColumnDef::new(column.as_str(), def.ty);
            if !def.nullable {
                col = col.not_null();
            }
            if def.unique {
                col = col.unique_key();
            }
            if let Some(d) = def.default {
                col = col.default(d);
            }
            // Only a single-column key is declared inline; that is what makes
            // an integral key an `INTEGER PRIMARY KEY` row id alias.
            let is_pk = E::PrimaryKey::from_column(column).is_some();
            if is_pk && single_pk {
                col = col.primary_key();
                if E::PrimaryKey::auto_increment() {
                    col = col.auto_increment();
                }
            }
            create = create.col(col);
        }
        if !single_pk && !pks.is_empty() {
            create = create.primary_key(pks.iter().map(IdenStatic::as_str));
        }
        for relation in E::Relation::iter() {
            if let Some(fk) = relation.def().foreign_key() {
                create = create.foreign_key(fk);
            }
        }
        if self.strict {
            create = create.strict();
        }
        create
    }

    /// Builds one `CREATE INDEX IF NOT EXISTS` statement per column marked `indexed`.
    ///
    /// Index names follow `idx-<table>-<column>` so that they are predictable
    /// for migrations that need to drop them later.
    pub fn create_index_from_entity<E: EntityTrait>(&self, _: E) -> Vec<CreateIndex> {
        E::Column::iter()
            .filter(|c| c.def().indexed)
            .map(|c| {
                CreateIndex::new()
                    .name(format!("idx-{}-{}", E::TABLE_NAME, c.as_str()))
                    .table(E::TABLE_NAME)
                    .col(c.as_str())
                    .if_not_exists()
            })
            .collect()
    }
}
