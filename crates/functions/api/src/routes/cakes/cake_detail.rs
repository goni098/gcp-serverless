use axum::Json;
use axum::extract::Path;
use database::models::Cake;
use database::repositories;
use http::exception::{HttpException, Rh};
use http::extractors::DbConnection;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Params {
    cake_id: i32,
}

pub async fn handler(
    DbConnection(mut connection): DbConnection,
    Path(params): Path<Params>,
) -> Rh<Json<Cake>> {
    let cake = repositories::cake::find_by_id(&mut connection, params.cake_id)
        .await?
        .ok_or_else(|| HttpException::bad_request("cake not found"))?;

    Ok(Json(cake))
}
