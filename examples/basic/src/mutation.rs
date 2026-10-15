//! Writes: insert, update, save, delete, the transactional variants and the
//! request-shaped inputs.
//!
//! The functions run in order and leave the data the query walkthrough
//! relies on: two providers, three models and a handful of agents.

use entity::prelude::{Agent, LlmModel, LlmProvider, Thread};
use entity::{agent, llm_model, llm_provider, thread};
use turso_orm::prelude::*;

/// Runs every mutation example in sequence.
pub async fn all_about_mutation(db: &Database) -> Result<(), DbErr> {
    insert_and_update(db).await?;
    println!("----- -----\n");
    save_and_delete(db).await?;
    println!("----- -----\n");
    bulk_and_transaction(db).await?;
    println!("----- -----\n");
    requests(db).await?;
    Ok(())
}

// --8<-- [start:insert_update]
/// Inserts a provider and a model, then updates the model through its active model.
async fn insert_and_update(db: &Database) -> Result<(), DbErr> {
    // `id` is left `NotSet`, so the database assigns it and `insert` hands
    // back the stored row.
    let provider = llm_provider::ActiveModel {
        name: Set("OpenAI".to_owned()),
        base_url: Set("https://api.openai.com/v1".to_owned()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    println!("inserted provider: {provider:?}");

    let model = llm_model::ActiveModel {
        name: Set("gpt-4.1-mini".to_owned()),
        input_price: Set(0.4),
        supports_tools: Set(true),
        capabilities: Set(Some(serde_json::json!({ "context_window": 128_000 }))),
        provider_id: Set(provider.id),
        ..Default::default()
    }
    .insert(db)
    .await?;
    println!("inserted model: {model:?}");

    // A model turned into an active model is entirely `Unchanged`; only the
    // fields set afterwards reach the `UPDATE` statement.
    let mut model: llm_model::ActiveModel = model.into();
    model.input_price = Set(0.5);
    let model = model.update(db).await?;
    println!("updated model: {model:?}");

    // A unique violation is classified, not just stringified.
    let dup = llm_provider::ActiveModel {
        name: Set("OpenAI".to_owned()),
        base_url: Set("https://example.invalid".to_owned()),
        ..Default::default()
    }
    .insert(db)
    .await;
    println!(
        "duplicate provider rejected as unique violation: {}",
        dup.as_ref().is_err_and(DbErr::is_unique_violation)
    );
    Ok(())
}
// --8<-- [end:insert_update]

// --8<-- [start:save_delete]
/// `save` inserts when the key is `NotSet` and updates otherwise; `delete` removes by key.
async fn save_and_delete(db: &Database) -> Result<(), DbErr> {
    let scratch = agent::ActiveModel {
        name: Set("scratch".to_owned()),
        ..Default::default()
    }
    .save(db)
    .await?;
    println!("saved (insert): {scratch:?}");

    let mut scratch = scratch;
    scratch.name = Set("scratch-pad".to_owned());
    let scratch = scratch.save(db).await?;
    println!("saved (update): {scratch:?}");

    let result = scratch.delete(db).await?;
    println!("deleted: {result:?}");
    Ok(())
}
// --8<-- [end:save_delete]

// --8<-- [start:bulk_transaction]
/// A multi-row insert with `RETURNING`, a bulk update and a closure transaction.
async fn bulk_and_transaction(db: &Database) -> Result<(), DbErr> {
    let openai = LlmProvider::find().one(db).await?.expect("seeded above");
    let anthropic = llm_provider::ActiveModel {
        name: Set("Anthropic".to_owned()),
        base_url: Set("https://api.anthropic.com".to_owned()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    let models = LlmModel::insert_many([
        llm_model::ActiveModel {
            name: Set("gpt-4.1".to_owned()),
            input_price: Set(2.0),
            supports_tools: Set(true),
            provider_id: Set(openai.id),
            ..Default::default()
        },
        llm_model::ActiveModel {
            name: Set("claude-sonnet-4".to_owned()),
            input_price: Set(3.0),
            supports_tools: Set(false),
            provider_id: Set(anthropic.id),
            ..Default::default()
        },
    ])
    .exec_with_returning(db)
    .await?;
    println!("inserted {} models", models.len());

    let agents = [
        ("planner", Some(1)),
        ("writer", Some(1)),
        ("reviewer", Some(2)),
        ("intern", None),
        ("archivist", None),
    ];
    let inserted = Agent::insert_many(agents.map(|(name, model_id)| agent::ActiveModel {
        name: Set(name.to_owned()),
        model_id: Set(model_id),
        ..Default::default()
    }))
    .exec(db)
    .await?;
    println!("inserted {inserted} agents");

    let discounted = LlmModel::update_many()
        .col(llm_model::Column::InputPrice, 2.5)
        .filter(llm_model::Column::SupportsTools.eq(false))
        .exec(db)
        .await?;
    println!("bulk update touched {} rows", discounted.rows_affected);

    // The closure form commits on `Ok` and rolls back on `Err`. Here the
    // insert is undone on purpose, so the agent count is unchanged after.
    let attempted: Result<(), DbErr> = db
        .transaction(|txn| {
            Box::pin(async move {
                agent::ActiveModel {
                    name: Set("ghost".to_owned()),
                    ..Default::default()
                }
                .insert(txn)
                .await?;
                Err(DbErr::Custom("abort on purpose".to_owned()))
            })
        })
        .await;
    println!("transaction result: {attempted:?}");
    println!("agents after rollback: {}", Agent::find().count(db).await?);
    Ok(())
}
// --8<-- [end:bulk_transaction]

// --8<-- [start:into_active_model]
/// The body of an "open thread" request: a subset of the columns, with an
/// optional reviewer that is only set when the client sends one.
#[derive(Debug, DeriveIntoActiveModel)]
#[turso(active_model = "thread::ActiveModel")]
struct OpenThread {
    agent_id: i32,
    title: String,
    reviewer_agent_id: Option<i32>,
}
// --8<-- [end:into_active_model]

// --8<-- [start:requests]
/// Opens threads from a request struct and from a JSON body, and deletes
/// them with `RETURNING`.
async fn requests(db: &Database) -> Result<(), DbErr> {
    let planner = Agent::find()
        .filter(agent::Column::Name.eq("planner"))
        .one(db)
        .await?
        .expect("seeded above");
    let opened = OpenThread {
        agent_id: planner.id,
        title: "From a request".to_owned(),
        reviewer_agent_id: None,
    }
    .into_active_model()
    .insert(db)
    .await?;
    println!("opened {:?} with status {:?}", opened.title, opened.status);

    let from_json = thread::ActiveModel::from_json(serde_json::json!({
        "agent_id": planner.id,
        "title": "From JSON",
        "status": "done"
    }))?
    .insert(db)
    .await?;
    println!(
        "opened {:?} with status {:?}",
        from_json.title, from_json.status
    );

    let removed = Thread::delete_many()
        .filter(thread::Column::Title.starts_with("From "))
        .exec_with_returning(db)
        .await?;
    println!("removed {} thread(s)", removed.len());
    Ok(())
}
// --8<-- [end:requests]
