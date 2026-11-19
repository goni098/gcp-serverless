use axum::Json;
use database::models::Cake;
use database::repositories;
use http::exception::Rh;
use http::extractors::DbConnection;
use serde::Serialize;

#[derive(Serialize)]
pub struct Response {
    cakes: Vec<Cake>,
}

pub async fn handler(DbConnection(mut connection): DbConnection) -> Rh<Json<Response>> {
    let cakes = repositories::cake::find_all(&mut connection).await?;
    let response = Response { cakes };
    Ok(Json(response))
}
