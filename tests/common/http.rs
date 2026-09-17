//! A test client that drives the real handlers through a router mirroring
//! `main.rs` (minus the rate limiter, compression and static file service).
#![allow(dead_code)]

use std::collections::BTreeMap;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
    routing::{get, post},
};
use axum_login::{AuthManagerLayerBuilder, login_required};
use cosmeredle::{
    backend::Backend,
    db,
    server::{day, handle_guess, handle_list, handle_login, handle_logout, handle_signup, home, me},
};
use serde_json::Value;
use time::Duration;
use tower::ServiceExt;
use tower_sessions::{Expiry, SessionManagerLayer, cookie::Key};
use tower_sessions_sqlx_store::SqliteStore;

pub async fn router() -> Router {
    let session_store = SqliteStore::new(db::conn().clone());
    session_store.migrate().await.expect("session store migration");

    let session_layer = SessionManagerLayer::new(session_store)
        .with_expiry(Expiry::OnInactivity(Duration::days(1)))
        .with_signed(Key::generate());

    let auth_layer = AuthManagerLayerBuilder::new(Backend, session_layer).build();

    Router::<()>::new()
        .route("/me", get(me))
        .route("/logout", post(handle_logout))
        .route_layer(login_required!(Backend))
        .route("/guess", post(handle_guess))
        .route("/list", get(handle_list))
        .route("/signup", post(handle_signup))
        .route("/login", post(handle_login))
        .route("/day", get(day))
        .route("/", get(home))
        .layer(auth_layer)
}

pub struct Response {
    pub status: StatusCode,
    pub body: String,
    pub content_type: Option<String>,
}

impl Response {
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body)
            .unwrap_or_else(|e| panic!("body is not json ({e}): {}", self.body))
    }
}

/// Keeps a cookie jar, so a login persists across calls like a browser's would.
pub struct Client {
    router: Router,
    cookies: BTreeMap<String, String>,
}

impl Client {
    pub async fn new() -> Self {
        Self {
            router: router().await,
            cookies: BTreeMap::new(),
        }
    }

    pub fn cookie_count(&self) -> usize {
        self.cookies.len()
    }

    pub async fn get(&mut self, uri: &str) -> Response {
        self.send(Method::GET, uri, None).await
    }

    pub async fn post(&mut self, uri: &str, body: &Value) -> Response {
        self.send(Method::POST, uri, Some(body.clone())).await
    }

    /// POST with no body at all, for the handlers that take no input.
    pub async fn post_empty(&mut self, uri: &str) -> Response {
        self.send(Method::POST, uri, None).await
    }

    pub async fn send(&mut self, method: Method, uri: &str, body: Option<Value>) -> Response {
        let mut req = Request::builder().method(method).uri(uri);

        if !self.cookies.is_empty() {
            let jar = self
                .cookies
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("; ");
            req = req.header(header::COOKIE, jar);
        }

        let req = match body {
            Some(v) => req
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&v).expect("serialize body")))
                .expect("request"),
            None => req.body(Body::empty()).expect("request"),
        };

        let res = self
            .router
            .clone()
            .oneshot(req)
            .await
            .expect("router is infallible");

        let status = res.status();
        for value in res.headers().get_all(header::SET_COOKIE) {
            let raw = value.to_str().expect("ascii cookie");
            let pair = raw.split(';').next().unwrap_or_default();
            if let Some((k, v)) = pair.split_once('=') {
                self.cookies.insert(k.trim().to_string(), v.trim().to_string());
            }
        }

        let content_type = res
            .headers()
            .get(header::CONTENT_TYPE)
            .map(|v| v.to_str().expect("ascii content type").to_string());

        let bytes = to_bytes(res.into_body(), usize::MAX).await.expect("body");
        Response {
            status,
            body: String::from_utf8_lossy(&bytes).into_owned(),
            content_type,
        }
    }
}

pub fn auth(username: &str, password: &str) -> Value {
    serde_json::json!({ "username": username, "password": password })
}
