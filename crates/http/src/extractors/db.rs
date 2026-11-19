use axum::extract::{FromRef, FromRequestParts};
use toasty::Connection;

use crate::exception::HttpException;

pub struct DbConnection(pub Connection);

impl<S> FromRequestParts<S> for DbConnection
where
    S: Send + Sync,
    toasty::Db: FromRef<S>,
{
    type Rejection = HttpException;

    async fn from_request_parts(
        _parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let db = toasty::Db::from_ref(state);
        let connection = db.connection().await?;

        Ok(Self(connection))
    }
}
