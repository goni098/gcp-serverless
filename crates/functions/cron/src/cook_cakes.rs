use database::repositories;
use error::R;

pub async fn run(db: toasty::Db) -> R<()> {
    let mut connection = db.connection().await?;
    repositories::cake::cook(&mut connection).await?;
    Ok(())
}
