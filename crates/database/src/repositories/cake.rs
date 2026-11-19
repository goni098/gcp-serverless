use error::R;
use jiff::{SignedDuration, Timestamp};
use toasty::Connection;

use crate::models::{Cake, Fruit};

pub async fn find_by_id(connection: &mut Connection, id: i32) -> R<Option<Cake>> {
    let cake = Cake::filter_by_id(id).first().exec(connection).await?;
    Ok(cake)
}

pub async fn find_all(connection: &mut Connection) -> R<Vec<Cake>> {
    let cakes = Cake::all().exec(connection).await?;
    Ok(cakes)
}

pub async fn cook(connection: &mut Connection) -> R<()> {
    toasty::create!(Cake::[
        {
            name: "Fruit tart",
            fruits: [
                { name: "Strawberry" },
                { name: "Mongo" },
                { name: "Kiwi" }
            ]
        },
        {
            name: "Mango Mousse",
            fruits: [
                { name: "Pasion fruit" },
                { name: "Mongo" }
            ]
        },
        {
            name: "Strawberry Shortcake",
            fruits: [
                { name: "Strawberry" },
                { name: "Mongo" },
                { name: "Kiwi" }
            ]
        }
    ])
    .exec(connection)
    .await?;

    Ok(())
}

pub async fn destroy_stale(connection: &mut Connection) -> R<()> {
    let cutoff = Timestamp::now().checked_sub(SignedDuration::from_hours(6))?;
    let mut transaction = connection.transaction().await?;

    Fruit::filter(
        Fruit::fields()
            .cake()
            .in_query(Cake::filter(Cake::fields().created_at().lt(cutoff))),
    )
    .delete()
    .exec(&mut transaction)
    .await?;

    Cake::filter(Cake::fields().created_at().lt(cutoff))
        .delete()
        .exec(&mut transaction)
        .await?;

    transaction.commit().await?;

    Ok(())
}
