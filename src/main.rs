use std::pin::Pin;

use axum::{
    Router,
    routing::{get, post},
};
use axum_login::{
    AuthManagerLayerBuilder,
    tower_sessions::{ExpiredDeletion, Expiry},
};
use cosmeredle::{
    Result,
    backend::Backend,
    db, err, init,
    server::{
        day, handle_guess, handle_list, handle_login, handle_logout, handle_signup, home, me,
    },
    wiki::sync_characters,
};
use futures::FutureExt;
use time::Duration;
use tokio_cron_scheduler::{Job, JobScheduler};
use tower_http::{
    compression::CompressionLayer, limit::RequestBodyLimitLayer, services::ServeDir,
    trace::TraceLayer,
};
use tower_sessions::{SessionManagerLayer, cookie::Key};
use tower_sessions_sqlx_store::SqliteStore;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    start().await
}

async fn start() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    init().await?;

    let sched = JobScheduler::new().await?;

    sched
        .add(Job::new_one_shot_async(
            std::time::Duration::default(),
            *Box::pin(update_cache_cron),
        )?)
        .await?;
    sched
        .add(Job::new_async("0 0 * * * *", update_cache_cron)?)
        .await?;

    sched.start().await?;

    let session_store = SqliteStore::new(db::conn().clone());
    session_store.migrate().await?;

    let deletion_task = tokio::task::spawn(
        session_store
            .clone()
            .continuously_delete_expired(tokio::time::Duration::from_secs(60)),
    );

    let key = Key::try_generate().ok_or(err!("Failed to generate signing key for sessions"))?;

    let session_layer = SessionManagerLayer::new(session_store)
        .with_expiry(Expiry::OnInactivity(Duration::days(1)))
        .with_signed(key);

    let backend = Backend;
    let auth_layer = AuthManagerLayerBuilder::new(backend, session_layer).build();

    let app = Router::<()>::new()
        .route("/guess", post(handle_guess))
        .route("/list", get(handle_list))
        .route("/signup", post(handle_signup))
        .route("/login", post(handle_login))
        .route("/logout", post(handle_logout))
        .route("/day", get(day))
        .route("/me", get(me))
        .route("/", get(home))
        .nest_service("/static", ServeDir::new("src/static"))
        .layer(CompressionLayer::new())
        .layer(RequestBodyLimitLayer::new(1024))
        .layer(TraceLayer::new_for_http())
        .layer(auth_layer);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    deletion_task.await??;

    Ok(())
}

fn update_cache_cron(_: uuid::Uuid, _: JobScheduler) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(sync_characters().map(|_| ()))
}
