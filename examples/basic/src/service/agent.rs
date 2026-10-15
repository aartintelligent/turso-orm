//! The agents: creation, the paged listing with its filter and sort, the
//! model they run on, the tools their threads used.

use crate::entity::{Agent, LlmModel};
use crate::entity::{agent, llm_model, thread, thread_tool, tool};
use futures_util::TryStreamExt;
use turso_orm::prelude::*;
use turso_orm::{ConnectionTrait, StreamTrait};

use crate::dto::agent::{AgentWithModel, AgentWithTools, CreateAgent, SaveAgent};
use crate::dto::base::pagination::{Page, PageRequest};

/// Inserts an agent from the request body.
pub async fn create_agent<C: ConnectionTrait>(
    db: &C,
    body: CreateAgent,
) -> Result<agent::Model, DbErr> {
    body.into_active_model().insert(db).await
}

// --8<-- [start:save_delete]
/// `save` inserts when the key is `NotSet` and updates when it is `Unchanged`.
pub async fn save_agent<C: ConnectionTrait>(
    db: &C,
    body: SaveAgent,
) -> Result<agent::Model, DbErr> {
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
pub async fn delete_agent<C: ConnectionTrait>(db: &C, id: i32) -> Result<u64, DbErr> {
    let result = Agent::delete_by_id(id).exec(db).await?;
    Ok(result.rows_affected)
}
// --8<-- [end:save_delete]

// --8<-- [start:listing]
/// Which agents a listing keeps; every criterion is optional and they add up.
#[derive(Debug, Clone, Default)]
pub struct AgentFilter {
    /// Part of the name.
    pub name: Option<String>,
    /// The model the agent is configured on.
    pub model_id: Option<i32>,
}

/// The column a listing is ordered by.
#[derive(Debug, Clone, Copy, Default)]
pub enum AgentSort {
    /// By key, the insertion order.
    #[default]
    Id,
    /// Alphabetically.
    Name,
}

/// A listing of agents: what to keep, how to order, which page.
#[derive(Debug, Clone)]
pub struct AgentQuery {
    /// What to keep.
    pub filter: AgentFilter,
    /// The column to order by.
    pub sort: AgentSort,
    /// The direction of the ordering.
    pub direction: Order,
    /// The page to return.
    pub page: PageRequest,
}

impl Default for AgentQuery {
    /// Everything, by key, smallest first, the first page.
    fn default() -> Self {
        Self {
            filter: AgentFilter::default(),
            sort: AgentSort::default(),
            direction: Order::Asc,
            page: PageRequest::default(),
        }
    }
}

/// The `WHERE` clause of a filter: one `AND` of the criteria present.
fn condition(filter: &AgentFilter) -> Condition {
    let mut all = Condition::all();
    if let Some(name) = &filter.name {
        all = all.add(agent::Column::Name.contains(name));
    }
    if let Some(model_id) = filter.model_id {
        all = all.add(agent::Column::ModelId.eq(model_id));
    }
    all
}

/// The column behind each sort: a new sort does not compile until it has one.
const fn sort_column(sort: AgentSort) -> agent::Column {
    match sort {
        AgentSort::Id => agent::Column::Id,
        AgentSort::Name => agent::Column::Name,
    }
}
// --8<-- [end:listing]

// --8<-- [start:paginate_stream]
/// One page of agents with the totals, each with its model: the page is one
/// query, the models of the whole page one more. The key breaks ties so
/// that a page is the same on every read.
pub async fn list_agents<C: ConnectionTrait>(
    db: &C,
    query: &AgentQuery,
) -> Result<Page<AgentWithModel>, DbErr> {
    let paginator = Agent::find()
        .filter(condition(&query.filter))
        .order_by(sort_column(query.sort), query.direction)
        .order_by_asc(agent::Column::Id)
        .paginate(db, query.page.size());
    let (total, pages) = paginator.num_items_and_pages().await?;
    let agents = paginator.fetch_page(query.page.page()).await?;
    let models = agents.load_one(LlmModel, db).await?;
    let items = agents
        .into_iter()
        .zip(models)
        .map(|(agent, model)| AgentWithModel { agent, model })
        .collect();
    Ok(Page::new(items, query.page, total, pages))
}

/// Every agent name, read one row at a time without holding the models.
pub async fn agent_names<C: StreamTrait>(db: &C) -> Result<Vec<String>, DbErr> {
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
pub async fn get_agent<C: ConnectionTrait>(
    db: &C,
    id: i32,
) -> Result<Option<AgentWithModel>, DbErr> {
    let Some(agent) = Agent::find_by_id(id).one(db).await? else {
        return Ok(None);
    };
    let model = agent.find_related(LlmModel).one(db).await?;
    Ok(Some(AgentWithModel { agent, model }))
}

/// Every agent with its model in one `SELECT`, through a `LEFT JOIN`.
pub async fn agents_with_models<C: ConnectionTrait>(db: &C) -> Result<Vec<AgentWithModel>, DbErr> {
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
pub async fn agents_on_models_above<C: ConnectionTrait>(
    db: &C,
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
pub async fn tools_of_agent<C: ConnectionTrait>(
    db: &C,
    id: i32,
) -> Result<Option<Vec<tool::Model>>, DbErr> {
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
pub async fn agents_with_tools<C: ConnectionTrait>(db: &C) -> Result<Vec<AgentWithTools>, DbErr> {
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
