use std::{pin::Pin, time::Duration};

use axum::{
    Router,
    routing::{get, post},
};
use cosmeredle::{
    Result,
    backend::{handle_guess, handle_list, home},
    cache::update_cache,
    init,
};
use futures::FutureExt;
use std::sync::mpsc::channel;
use tokio::task::spawn_blocking;
use tokio_cron_scheduler::{Job, JobScheduler};
use tower_http::{compression::CompressionLayer, limit::RequestBodyLimitLayer};

#[tokio::main]
async fn main() -> Result<()> {
    start().await
}

async fn start() -> Result<()> {
    init().await?;

    let (send, recv) = channel();
    ctrlc::set_handler(move || send.clone().send(()).unwrap()).unwrap();

    let sched = JobScheduler::new().await?;

    sched
        .add(Job::new_one_shot_async(
            Duration::default(),
            *Box::pin(update_cache_cron),
        )?)
        .await?;
    sched
        .add(Job::new_async("0 0 * * * *", update_cache_cron)?)
        .await?;

    sched.start().await?;

    let app = Router::<()>::new()
        .layer(CompressionLayer::new())
        .layer(RequestBodyLimitLayer::new(1024))
        .route("/guess", post(handle_guess))
        .route("/list", get(handle_list))
        .route("/", get(home));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(spawn_blocking(move || recv.recv().unwrap()).map(|_| ()))
        .await
        .unwrap();

    Ok(())
}

fn update_cache_cron(_: uuid::Uuid, _: JobScheduler) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(update_cache().map(|_| ()))
}
