//! Reads: finders, filters, ordering, pagination, streaming, projections and relations.
//!
//! Every function prints the models it reads so that the console output
//! mirrors the code.

use futures_util::TryStreamExt;
use turso_orm::prelude::*;

use crate::entity::{Bakery, Cake, Fruit, cake, fruit};

/// Runs every query example in sequence.
pub async fn all_about_query(db: &Database) -> Result<(), DbErr> {
    find_all(db).await?;
    println!("----- -----\n");
    find_one_and_filter(db).await?;
    println!("----- -----\n");
    paginate_and_stream(db).await?;
    println!("----- -----\n");
    project_into_struct(db).await?;
    println!("----- -----\n");
    relations(db).await?;
    println!("----- -----\n");
    loaders(db).await?;
    Ok(())
}

// --8<-- [start:find_all]
/// Lists every row of two tables.
async fn find_all(db: &Database) -> Result<(), DbErr> {
    println!("find all cakes:");
    for cake in Cake::find().all(db).await? {
        println!("  {cake:?}");
    }
    println!("find all fruits:");
    for fruit in Fruit::find().all(db).await? {
        println!("  {fruit:?}");
    }
    Ok(())
}
// --8<-- [end:find_all]

// --8<-- [start:filter]
/// Finds by key, by `LIKE`, and with a composed `OR` condition.
async fn find_one_and_filter(db: &Database) -> Result<(), DbErr> {
    let by_id = Cake::find_by_id(1).one(db).await?;
    println!("find one by primary key: {by_id:?}");

    let by_name = Cake::find()
        .filter(cake::Column::Name.contains("chocolate"))
        .one(db)
        .await?;
    println!("find one by name: {by_name:?}");

    let cond = Condition::any()
        .add(cake::Column::GlutenFree.eq(true))
        .add(cake::Column::Price.gt(10));
    let cakes = Cake::find()
        .filter(cond)
        .order_by_desc(cake::Column::Price)
        .all(db)
        .await?;
    println!("gluten free or pricey, most expensive first:");
    for cake in cakes {
        println!("  {cake:?}");
    }
    Ok(())
}
// --8<-- [end:filter]

// --8<-- [start:paginate_stream]
/// Pages through fruits two at a time, then streams them one row at a time.
async fn paginate_and_stream(db: &Database) -> Result<(), DbErr> {
    let paginator = Fruit::find()
        .order_by_asc(fruit::Column::Id)
        .paginate(db, 2);
    let (items, pages) = paginator.num_items_and_pages().await?;
    println!("fruits: {items} items over {pages} pages");
    for page in 0..pages {
        println!("  page {page}: {:?}", paginator.fetch_page(page).await?);
    }

    println!("stream of fruit names:");
    let mut stream = Fruit::find()
        .order_by_asc(fruit::Column::Name)
        .stream(db)
        .await?;
    while let Some(fruit) = stream.try_next().await? {
        println!("  {}", fruit.name);
    }
    Ok(())
}
// --8<-- [end:paginate_stream]

// --8<-- [start:project_struct]
/// Aggregates per bakery decoded into a plain struct.
#[derive(Debug, FromQueryResult)]
struct CakeStats {
    /// Number of cakes.
    count: i64,
    /// Highest price.
    #[turso(column_name = "max_price")]
    most_expensive: f64,
}
// --8<-- [end:project_struct]

// --8<-- [start:project]
/// Selects only computed columns and decodes them by name.
async fn project_into_struct(db: &Database) -> Result<(), DbErr> {
    let stats = Cake::find()
        .select_only()
        .expr_as(Func::count_star(), "count")
        .expr_as(cake::Column::Price.max(), "max_price")
        .into_model::<CakeStats>()
        .one(db)
        .await?;
    if let Some(CakeStats {
        count,
        most_expensive,
    }) = stats
    {
        println!("{count} cakes, the most expensive at {most_expensive}");
    }
    Ok(())
}
// --8<-- [end:project]

// --8<-- [start:relations]
/// Walks relations in both directions, with one query per hop or a single join.
async fn relations(db: &Database) -> Result<(), DbErr> {
    let bakery = Bakery::find().one(db).await?.expect("seeded");
    println!("cakes of {}:", bakery.name);
    for cake in bakery.find_related(Cake).all(db).await? {
        println!("  {}", cake.name);
    }

    let fruit = Fruit::find()
        .order_by_asc(fruit::Column::Id)
        .one(db)
        .await?
        .expect("seeded");
    let cake = fruit.find_related(Cake).one(db).await?;
    println!("{} sits on {:?}", fruit.name, cake.map(|c| c.name));

    println!("every fruit with its cake in one SELECT:");
    for (fruit, cake) in Fruit::find()
        .find_also_related(Cake)
        .order_by(fruit::Column::Id, Order::Asc)
        .all(db)
        .await?
    {
        println!("  {} -> {:?}", fruit.name, cake.map(|c| c.name));
    }

    println!("fruits on cakes above 9.0 (inner join, filter on the joined table):");
    for fruit in Fruit::find()
        .inner_join(Cake)
        .filter(cake::Column::Price.gt(9.0))
        .all(db)
        .await?
    {
        println!("  {}", fruit.name);
    }
    Ok(())
}
// --8<-- [end:relations]

// --8<-- [start:loaders]
/// Batch loaders: one `IN (...)` query for many parents, results aligned with the input.
async fn loaders(db: &Database) -> Result<(), DbErr> {
    let cakes = Cake::find().order_by_asc(cake::Column::Id).all(db).await?;
    let fruits = cakes.load_many(Fruit, db).await?;
    println!("fruits per cake through load_many:");
    for (cake, fruits) in cakes.iter().zip(&fruits) {
        let names: Vec<&str> = fruits.iter().map(|f| f.name.as_str()).collect();
        println!("  {}: {names:?}", cake.name);
    }

    let bakeries = cakes.load_one(Bakery, db).await?;
    println!("bakery per cake through load_one:");
    for (cake, bakery) in cakes.iter().zip(&bakeries) {
        println!("  {} <- {:?}", cake.name, bakery.as_ref().map(|b| &b.name));
    }
    Ok(())
}
// --8<-- [end:loaders]
