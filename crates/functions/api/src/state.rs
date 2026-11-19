use axum::extract::FromRef;
use error::R;
use toasty::Db;

#[derive(Clone, FromRef)]
pub struct AppState {
    pub db: Db,
}

impl AppState {
    pub async fn new(db_url: &str) -> R<Self> {
        let db = database::connect_with_retry(db_url, None, None).await?;
        let state = AppState { db };

        Ok(state)
    }
}
