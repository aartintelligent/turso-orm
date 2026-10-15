//! The models: inserts and updates, filters, bulk writes, aggregates and the
//! agents of each model.

use crate::entity::{Agent, LlmModel, llm_model};
use turso_orm::ConnectionTrait;
use turso_orm::prelude::*;

use crate::dto::llm_model::{CreateModel, ModelFilter, ModelStats, ModelWithAgents};

// --8<-- [start:insert_update]
/// Inserts a model, leaving `id` to the database, and hands back the stored row.
pub async fn create_model<C: ConnectionTrait>(
    db: &C,
    body: CreateModel,
) -> Result<llm_model::Model, DbErr> {
    // `id` is left `NotSet`, so the database assigns it and `insert` returns
    // the stored row.
    body.into_active_model().insert(db).await
}

/// Changes the price of one model; `None` when the model does not exist.
pub async fn update_price<C: ConnectionTrait>(
    db: &C,
    id: i32,
    input_price: f64,
) -> Result<Option<llm_model::Model>, DbErr> {
    let Some(model) = LlmModel::find_by_id(id).one(db).await? else {
        return Ok(None);
    };
    // A model turned into an active model is entirely `Unchanged`; only the
    // fields set afterwards reach the `UPDATE` statement.
    let mut model: llm_model::ActiveModel = model.into();
    model.input_price = Set(input_price);
    model.update(db).await.map(Some)
}
// --8<-- [end:insert_update]

// --8<-- [start:filter]
/// One model by primary key.
pub async fn find_model<C: ConnectionTrait>(
    db: &C,
    id: i32,
) -> Result<Option<llm_model::Model>, DbErr> {
    LlmModel::find_by_id(id).one(db).await
}

/// The models matching the filter: a `LIKE` on the name, and an `OR` of the
/// two other criteria, most expensive first.
pub async fn search_models<C: ConnectionTrait>(
    db: &C,
    filter: &ModelFilter,
) -> Result<Vec<llm_model::Model>, DbErr> {
    let mut query = LlmModel::find();
    if let Some(name) = &filter.name {
        query = query.filter(llm_model::Column::Name.contains(name));
    }
    let mut any = Condition::any();
    if let Some(tools) = filter.tools {
        any = any.add(llm_model::Column::SupportsTools.eq(tools));
    }
    if let Some(min_price) = filter.min_price {
        any = any.add(llm_model::Column::InputPrice.gt(min_price));
    }
    query
        .filter(any)
        .order_by_desc(llm_model::Column::InputPrice)
        .all(db)
        .await
}
// --8<-- [end:filter]

// --8<-- [start:bulk]
/// Inserts several models in one statement and returns the stored rows.
pub async fn create_models<C: ConnectionTrait>(
    db: &C,
    bodies: Vec<CreateModel>,
) -> Result<Vec<llm_model::Model>, DbErr> {
    LlmModel::insert_many(bodies.into_iter().map(IntoActiveModel::into_active_model))
        .exec_with_returning(db)
        .await
}

/// Sets one price on every model that cannot call tools; returns how many changed.
pub async fn discount_tool_less_models<C: ConnectionTrait>(
    db: &C,
    input_price: f64,
) -> Result<u64, DbErr> {
    let result = LlmModel::update_many()
        .col(llm_model::Column::InputPrice, input_price)
        .filter(llm_model::Column::SupportsTools.eq(false))
        .exec(db)
        .await?;
    Ok(result.rows_affected)
}

/// Deletes the models priced below `input_price` and returns them as they were.
pub async fn remove_models_below<C: ConnectionTrait>(
    db: &C,
    input_price: f64,
) -> Result<Vec<llm_model::Model>, DbErr> {
    LlmModel::delete_many()
        .filter(llm_model::Column::InputPrice.lt(input_price))
        .exec_with_returning(db)
        .await
}
// --8<-- [end:bulk]

// --8<-- [start:project]
/// Selects only computed columns and decodes them by name.
pub async fn stats<C: ConnectionTrait>(db: &C) -> Result<Option<ModelStats>, DbErr> {
    LlmModel::find()
        .select_only()
        .expr_as(Func::count_star(), "count")
        .expr_as(llm_model::Column::InputPrice.max(), "max_price")
        .into_model::<ModelStats>()
        .one(db)
        .await
}
// --8<-- [end:project]

// --8<-- [start:loaders]
/// Every model with its agents: two queries whatever the number of models,
/// the loader answering aligned with its input.
pub async fn models_with_agents<C: ConnectionTrait>(db: &C) -> Result<Vec<ModelWithAgents>, DbErr> {
    let models = LlmModel::find()
        .order_by_asc(llm_model::Column::Id)
        .all(db)
        .await?;
    let agents = models.load_many(Agent, db).await?;
    Ok(models
        .into_iter()
        .zip(agents)
        .map(|(model, agents)| ModelWithAgents { model, agents })
        .collect())
}
// --8<-- [end:loaders]
