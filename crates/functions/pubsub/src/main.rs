use std::net::Ipv4Addr;

use axum::http::StatusCode;
use axum::routing::post;
use axum::{Router, middleware};
use http::exception::Rh;
use http::extractors::PubsubEvent;
use http::middlewares::error_reporter;
use shared::env::Env;

async fn handler(event: PubsubEvent) -> Rh<StatusCode> {
    tracing::info!("Pubsub event: {:#?}", event);
    Ok(StatusCode::NO_CONTENT)
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    shared::tracing::install();
    shared::env::load();

    let port = Env::Port.read_to::<u16>().expect("invalid port");
    let addr = (Ipv4Addr::UNSPECIFIED, port);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    let app = Router::new()
        .route("/", post(handler))
        .layer(middleware::from_fn(error_reporter));

    tracing::info!(%port, "Listening on 0.0.0.0");

    axum::serve(listener, app)
        .with_graceful_shutdown(shared::functions::graceful_shutdown())
        .await?;

    tracing::info!("Server stopped");

    Ok(())
}
