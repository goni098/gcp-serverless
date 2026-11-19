use database::repositories;
use error::R;
use google_cloud_pubsub::client::Publisher;
use google_cloud_pubsub::model::Message;

pub async fn run(db: toasty::Db, topic: &str) -> R<()> {
    let publisher = Publisher::builder(topic).build().await?;

    let mut connection = db.connection().await?;

    repositories::cake::destroy_stale(&mut connection).await?;

    let message = Message::new().set_data("deleted stale cakes");

    publisher.publish(message).await?;

    Ok(())
}
