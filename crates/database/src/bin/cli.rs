use error::R;
use toasty_cli::{Config, ToastyCli};

static _MIGRATIONS: toasty::migration::MigrationSet = toasty::embed_migrations!("toasty");

#[tokio::main]
async fn main() -> R<()> {
    shared::tracing::install();
    shared::env::load();
    let config = Config::load().expect("can not load toasty config");
    let database_url = shared::env::Env::DatabaseUrl.read()?;
    let db = database::connect_with_retry(&database_url, None, None).await?;

    ToastyCli::with_config(db, config)
        .parse_and_run()
        .await
        .expect("toasty_cli got failed");

    Ok(())
}
