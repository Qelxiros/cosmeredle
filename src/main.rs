use axum::{Router, routing::post};
use cosmeredle::{
    Result,
    backend::handle_guess,
    cache::{load_cache, update_cache},
    init,
};

#[tokio::main]
async fn main() -> Result<()> {
    let result = start().await;

    shutdown();

    result
}

async fn start() -> Result<()> {
    dotenvy::dotenv()?;
    init().await?;

    // update_cache().await?;
    load_cache().await?;

    let app = Router::<()>::new().route("/guess", post(handle_guess));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

fn shutdown() {}
