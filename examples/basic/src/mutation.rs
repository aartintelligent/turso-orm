//! Writes: insert, update, save, delete and the transactional variants.
//!
//! The functions run in order and leave the data the query walkthrough
//! relies on: one bakery, three cakes and a handful of fruits.

use turso_orm::prelude::*;

use crate::entity::{Bakery, Cake, Fruit, bakery, cake, fruit};

/// Runs every mutation example in sequence.
pub async fn all_about_mutation(db: &Database) -> Result<(), DbErr> {
    insert_and_update(db).await?;
    println!("----- -----\n");
    save_and_delete(db).await?;
    println!("----- -----\n");
    bulk_and_transaction(db).await?;
    Ok(())
}

// --8<-- [start:insert_update]
/// Inserts a bakery and a cake, then updates the cake through its active model.
async fn insert_and_update(db: &Database) -> Result<(), DbErr> {
    // `id` is left `NotSet`, so the database assigns it and `insert` hands
    // back the stored row.
    let bakery = bakery::ActiveModel {
        name: Set("SeaSide Bakery".to_owned()),
        profit_margin: Set(10.4),
        ..Default::default()
    }
    .insert(db)
    .await?;
    println!("inserted bakery: {bakery:?}");

    let cake = cake::ActiveModel {
        name: Set("New York Cheese".to_owned()),
        price: Set(10.25),
        gluten_free: Set(false),
        attributes: Set(Some(serde_json::json!({ "layers": 2 }))),
        bakery_id: Set(bakery.id),
        ..Default::default()
    }
    .insert(db)
    .await?;
    println!("inserted cake: {cake:?}");

    // A model turned into an active model is entirely `Unchanged`; only the
    // fields set afterwards reach the `UPDATE` statement.
    let mut cake: cake::ActiveModel = cake.into();
    cake.price = Set(11.0);
    let cake = cake.update(db).await?;
    println!("updated cake: {cake:?}");

    // A unique violation is classified, not just stringified.
    let dup = bakery::ActiveModel {
        name: Set("SeaSide Bakery".to_owned()),
        profit_margin: Set(0.0),
        ..Default::default()
    }
    .insert(db)
    .await;
    println!(
        "duplicate bakery rejected as unique violation: {}",
        dup.as_ref().is_err_and(DbErr::is_unique_violation)
    );
    Ok(())
}
// --8<-- [end:insert_update]

// --8<-- [start:save_delete]
/// `save` inserts when the key is `NotSet` and updates otherwise; `delete` removes by key.
async fn save_and_delete(db: &Database) -> Result<(), DbErr> {
    let banana = fruit::ActiveModel {
        name: Set("Banana".to_owned()),
        ..Default::default()
    }
    .save(db)
    .await?;
    println!("saved (insert): {banana:?}");

    let mut banana = banana;
    banana.name = Set("Banana Mango".to_owned());
    let banana = banana.save(db).await?;
    println!("saved (update): {banana:?}");

    let result = banana.delete(db).await?;
    println!("deleted: {result:?}");
    Ok(())
}
// --8<-- [end:save_delete]

// --8<-- [start:bulk_transaction]
/// A multi-row insert with `RETURNING`, a bulk update and a closure transaction.
async fn bulk_and_transaction(db: &Database) -> Result<(), DbErr> {
    let bakery = Bakery::find().one(db).await?.expect("seeded above");
    let cakes = Cake::insert_many([
        cake::ActiveModel {
            name: Set("Chocolate Forest".to_owned()),
            price: Set(8.5),
            gluten_free: Set(false),
            bakery_id: Set(bakery.id),
            ..Default::default()
        },
        cake::ActiveModel {
            name: Set("Lemon Drizzle".to_owned()),
            price: Set(6.0),
            gluten_free: Set(true),
            bakery_id: Set(bakery.id),
            ..Default::default()
        },
    ])
    .exec_with_returning(db)
    .await?;
    println!("inserted {} cakes", cakes.len());

    let fruits = [
        ("Blueberry", Some(1)),
        ("Raspberry", Some(1)),
        ("Strawberry", Some(2)),
        ("Apple", None),
        ("Cherry", None),
    ];
    let inserted = Fruit::insert_many(fruits.map(|(name, cake_id)| fruit::ActiveModel {
        name: Set(name.to_owned()),
        cake_id: Set(cake_id),
        ..Default::default()
    }))
    .exec(db)
    .await?;
    println!("inserted {inserted} fruits");

    let bumped = Cake::update_many()
        .col(cake::Column::Price, 7.0)
        .filter(cake::Column::GlutenFree.eq(true))
        .exec(db)
        .await?;
    println!("bulk update touched {} rows", bumped.rows_affected);

    // The closure form commits on `Ok` and rolls back on `Err`. Here the
    // insert is undone on purpose, so the fruit count is unchanged after.
    let attempted: Result<(), DbErr> = db
        .transaction(|txn| {
            Box::pin(async move {
                fruit::ActiveModel {
                    name: Set("Ghost".to_owned()),
                    ..Default::default()
                }
                .insert(txn)
                .await?;
                Err(DbErr::Custom("abort on purpose".to_owned()))
            })
        })
        .await;
    println!("transaction result: {attempted:?}");
    println!("fruits after rollback: {}", Fruit::find().count(db).await?);
    Ok(())
}
// --8<-- [end:bulk_transaction]
