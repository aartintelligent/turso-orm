//! The agents: creation, listing, the model they run on, the tools they used.

use crate::entity::{Agent, LlmModel};
use crate::entity::{agent, llm_model, thread, thread_tool, tool};
use futures_util::TryStreamExt;
use turso_orm::prelude::*;

use crate::dto::agent::{AgentWithModel, AgentWithTools, CreateAgent, SaveAgent};
use crate::dto::page::{Page, Paged};

/// Inserts an agent from the request body.
pub async fn create_agent(db: &Database, body: CreateAgent) -> Result<agent::Model, DbErr> {
    body.into_active_model().insert(db).await
}

// --8<-- [start:save_delete]
/// `save` inserts when the key is `NotSet` and updates when it is `Unchanged`.
pub async fn save_agent(db: &Database, body: SaveAgent) -> Result<agent::Model, DbErr> {
    let mut draft = agent::ActiveModel {
        name: Set(body.name),
        ..Default::default()
    };
    if let Some(id) = body.id {
        draft.id = Unchanged(id);
    }
    let saved = draft.save(db).await?;
    saved.try_into_model()
}

/// Deletes an agent by key; the engine cascades to its threads.
pub async fn delete_agent(db: &Database, id: i32) -> Result<u64, DbErr> {
    let result = Agent::delete_by_id(id).exec(db).await?;
    Ok(result.rows_affected)
}
// --8<-- [end:save_delete]

// --8<-- [start:paginate_stream]
/// One page of agents with the totals, in id order, each with its model: the
/// page is one query, the models of the whole page one more.
pub async fn list_agents(db: &Database, page: Page) -> Result<Paged<AgentWithModel>, DbErr> {
    let paginator = Agent::find()
        .order_by_asc(agent::Column::Id)
        .paginate(db, page.size.max(1));
    let (total, pages) = paginator.num_items_and_pages().await?;
    let agents = paginator.fetch_page(page.page).await?;
    let models = agents.load_one(LlmModel, db).await?;
    let items = agents
        .into_iter()
        .zip(models)
        .map(|(agent, model)| AgentWithModel { agent, model })
        .collect();
    Ok(Paged {
        items,
        page: page.page,
        pages,
        total,
    })
}

/// Every agent name, read one row at a time without holding the models.
pub async fn agent_names(db: &Database) -> Result<Vec<String>, DbErr> {
    let mut stream = Agent::find()
        .order_by_asc(agent::Column::Name)
        .stream(db)
        .await?;
    let mut names = Vec::new();
    while let Some(agent) = stream.try_next().await? {
        names.push(agent.name);
    }
    Ok(names)
}
// --8<-- [end:paginate_stream]

// --8<-- [start:relations]
/// One agent with its model: one query for the agent, one through the relation.
pub async fn get_agent(db: &Database, id: i32) -> Result<Option<AgentWithModel>, DbErr> {
    let Some(agent) = Agent::find_by_id(id).one(db).await? else {
        return Ok(None);
    };
    let model = agent.find_related(LlmModel).one(db).await?;
    Ok(Some(AgentWithModel { agent, model }))
}

/// Every agent with its model in one `SELECT`, through a `LEFT JOIN`.
pub async fn agents_with_models(db: &Database) -> Result<Vec<AgentWithModel>, DbErr> {
    let pairs = Agent::find()
        .find_also_related(LlmModel)
        .order_by(agent::Column::Id, Order::Asc)
        .all(db)
        .await?;
    Ok(pairs
        .into_iter()
        .map(|(agent, model)| AgentWithModel { agent, model })
        .collect())
}

/// The agents whose model is priced above `min_price`: an inner join, and a
/// filter on the joined table.
pub async fn agents_on_models_above(
    db: &Database,
    min_price: f64,
) -> Result<Vec<agent::Model>, DbErr> {
    Agent::find()
        .inner_join(LlmModel)
        .filter(llm_model::Column::InputPrice.gt(min_price))
        .all(db)
        .await
}
// --8<-- [end:relations]

// --8<-- [start:linked]
/// The chain from an agent to the tools its threads invoked: three hops,
/// each an existing relation definition.
pub struct AgentToTool;

impl Linked for AgentToTool {
    type FromEntity = agent::Entity;
    type ToEntity = tool::Entity;

    fn link(&self) -> Vec<RelationDef> {
        vec![
            agent::Relation::Thread.def(),
            thread::Relation::ThreadTool.def(),
            thread_tool::Relation::Tool.def(),
        ]
    }
}
// --8<-- [end:linked]

// --8<-- [start:find_linked]
/// The distinct tools an agent's threads invoked, in one query through the chain.
pub async fn tools_of_agent(db: &Database, id: i32) -> Result<Option<Vec<tool::Model>>, DbErr> {
    let Some(agent) = Agent::find_by_id(id).one(db).await? else {
        return Ok(None);
    };
    agent
        .find_linked(AgentToTool)
        .distinct()
        .all(db)
        .await
        .map(Some)
}

/// Every agent with the tools its threads invoked, grouped from one query.
pub async fn agents_with_tools(db: &Database) -> Result<Vec<AgentWithTools>, DbErr> {
    let groups = Agent::find()
        .find_with_linked(&AgentToTool)
        .order_by(agent::Column::Id, Order::Asc)
        .all(db)
        .await?;
    Ok(groups
        .into_iter()
        .map(|(agent, tools)| AgentWithTools { agent, tools })
        .collect())
}
// --8<-- [end:find_linked]
