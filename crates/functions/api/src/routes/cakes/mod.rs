use axum::{Router, routing};

use crate::state::AppState;

mod cake_detail;
mod cake_list;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/cakes", routing::get(cake_list::handler))
        .route("/cakes/{cake_id}", routing::get(cake_detail::handler))
}
