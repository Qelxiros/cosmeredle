//! End-to-end tests for the signup/login/logout/session handlers and the
//! unauthenticated endpoints, driven through a router that mirrors `main.rs`.

mod common;

use axum::http::StatusCode;
use common::http::{Client, auth};
use cosmeredle::server::User;
use serde_json::json;

async fn client() -> Client {
    common::setup().await;
    Client::new().await
}

#[tokio::test]
async fn day_returns_the_local_date() {
    let mut c = client().await;
    let res = c.get("/day").await;

    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body, common::today_str());
}

#[tokio::test]
async fn home_serves_the_index_page() {
    let mut c = client().await;
    let res = c.get("/").await;

    assert_eq!(res.status, StatusCode::OK);
    assert!(res.body.starts_with("<!doctype html>"), "{}", res.body);
}

#[tokio::test]
async fn list_is_public_and_returns_the_playable_characters() {
    let mut c = client().await;
    common::insert_cosmere("Listed Hero", "Roshar", "Human", "Oathbringer", &[]).await;

    let res = c.get("/list").await;
    assert_eq!(res.status, StatusCode::OK);

    let names: Vec<String> = serde_json::from_value(res.json()).expect("list of names");
    assert!(names.contains(&"Listed Hero".to_string()));
}

#[tokio::test]
async fn me_requires_a_session() {
    let mut c = client().await;
    assert_eq!(c.get("/me").await.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn logout_requires_a_session() {
    let mut c = client().await;
    assert_eq!(c.post_empty("/logout").await.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn signup_logs_the_new_user_straight_in() {
    let mut c = client().await;

    let res = c.post("/signup", &auth("signup-user", "hunter2-long")).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body, "signup-user");
    assert!(c.cookie_count() > 0, "signup should set a session cookie");

    let me = c.get("/me").await;
    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(me.json(), json!({ "username": "signup-user", "guesses": [] }));
}

#[tokio::test]
async fn signup_stores_a_bcrypt_hash_rather_than_the_password() {
    let mut c = client().await;
    c.post("/signup", &auth("hash-user", "plaintext-password")).await;

    let stored = common::scalar("SELECT bcrypt FROM user WHERE username = 'hash-user'")
        .await
        .expect("user row");
    assert!(stored.starts_with("$2"), "not a bcrypt hash: {stored}");
    assert!(!stored.contains("plaintext-password"));
}

#[tokio::test]
async fn signing_up_twice_with_the_same_name_is_a_conflict() {
    let mut c = client().await;
    assert_eq!(
        c.post("/signup", &auth("dupe-user", "password123")).await.status,
        StatusCode::OK
    );

    let res = c.post("/signup", &auth("dupe-user", "different-password")).await;
    assert_eq!(res.status, StatusCode::CONFLICT);

    // The first user's password still works.
    let mut fresh = client().await;
    assert_eq!(
        fresh.post("/login", &auth("dupe-user", "password123")).await.status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn login_rejects_a_wrong_password() {
    let mut c = client().await;
    c.post("/signup", &auth("wrong-pw-user", "correct-password")).await;

    let mut fresh = client().await;
    let res = fresh.post("/login", &auth("wrong-pw-user", "incorrect-password")).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(res.body, "incorrect username or password");
    assert_eq!(fresh.get("/me").await.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn login_rejects_an_unknown_user() {
    let mut c = client().await;
    let res = c.post("/login", &auth("no-such-user", "whatever")).await;

    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        res.body, "incorrect username or password",
        "an unknown user must be indistinguishable from a wrong password"
    );
}

#[tokio::test]
async fn login_is_case_sensitive() {
    let mut c = client().await;
    c.post("/signup", &auth("CaseUser", "password123")).await;

    let mut fresh = client().await;
    assert_eq!(
        fresh.post("/login", &auth("caseuser", "password123")).await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn login_then_logout_ends_the_session() {
    let mut c = client().await;
    c.post("/signup", &auth("logout-user", "password123")).await;

    let mut fresh = client().await;
    assert_eq!(
        fresh.post("/login", &auth("logout-user", "password123")).await.status,
        StatusCode::OK
    );
    assert_eq!(fresh.get("/me").await.status, StatusCode::OK);

    assert_eq!(fresh.post_empty("/logout").await.status, StatusCode::OK);
    assert_eq!(
        fresh.get("/me").await.status,
        StatusCode::UNAUTHORIZED,
        "the session should be gone after logout"
    );
}

#[tokio::test]
async fn a_session_cookie_is_required_not_just_a_username() {
    let mut c = client().await;
    c.post("/signup", &auth("cookieless", "password123")).await;

    // A second client never saw the Set-Cookie header.
    let mut other = client().await;
    assert_eq!(other.get("/me").await.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn signup_validation_failures_explain_themselves() {
    let mut c = client().await;

    let res = c.post("/signup", &auth("ab", "password123")).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(res.body, "Username too short");

    let res = c.post("/signup", &auth("long-enough", "short")).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(res.body, "Password too short");
}

/// The username is checked before the password, so a request that fails both
/// only hears about the username.
#[tokio::test]
async fn the_username_is_validated_first() {
    let mut c = client().await;
    let res = c.post("/signup", &auth("ab", "short")).await;

    assert_eq!(res.body, "Username too short");
}

#[tokio::test]
async fn signup_requires_a_username_of_at_least_three_bytes() {
    let mut c = client().await;

    for name in ["", "a", "ab"] {
        let res = c.post("/signup", &auth(name, "password123")).await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{name:?} should be rejected");
        assert_eq!(res.body, "Username too short");
        assert_eq!(
            common::count(&format!(
                "SELECT count(*) FROM user WHERE username = '{name}'"
            ))
            .await,
            0,
            "{name:?} should not have been created"
        );
    }

    assert_eq!(
        c.post("/signup", &auth("abc", "password123")).await.status,
        StatusCode::OK,
        "three characters is the shortest accepted username"
    );
}

#[tokio::test]
async fn signup_requires_a_password_of_at_least_eight_bytes() {
    let mut c = client().await;

    for password in ["", "short", "7chars7"] {
        let res = c.post("/signup", &auth("pw-length", password)).await;
        assert_eq!(
            res.status,
            StatusCode::BAD_REQUEST,
            "{password:?} should be rejected"
        );
        assert_eq!(res.body, "Password too short");

        assert_eq!(
            common::count("SELECT count(*) FROM user WHERE username = 'pw-length'").await,
            0,
            "no account should exist yet"
        );
    }

    assert_eq!(
        c.post("/signup", &auth("pw-length", "8charsxx")).await.status,
        StatusCode::OK,
        "eight characters is the shortest accepted password"
    );
}

/// The checks are on `len()`, which counts bytes, so a short multi-byte
/// username or password passes.
#[tokio::test]
async fn validation_counts_bytes_not_characters() {
    let mut c = client().await;

    // Two characters, four bytes.
    let res = c.post("/signup", &auth("üü", "pässwörd")).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body, "üü");
}

#[tokio::test]
async fn malformed_bodies_are_rejected_by_the_extractor() {
    let mut c = client().await;

    let res = c.post("/login", &json!({ "username": "only-a-name" })).await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);

    let res = c.post("/signup", &json!("not an object")).await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn me_deserialises_into_the_handler_s_own_type() {
    let mut c = client().await;
    c.post("/signup", &auth("typed-me", "password123")).await;

    let parsed: User =
        serde_json::from_str(&c.get("/me").await.body).expect("the wire format matches User");
    assert_eq!(parsed.username, "typed-me");
    assert!(parsed.guesses.is_empty());
}

#[tokio::test]
async fn me_reports_only_todays_guesses() {
    let mut c = client().await;
    c.post("/signup", &auth("me-guesses", "password123")).await;

    let id: i64 = common::count("SELECT id FROM user WHERE username = 'me-guesses'").await;
    let old = common::date_str(common::days_ago(3));
    common::exec(&format!(
        "INSERT INTO guess (user_id, guess, day) VALUES
         ({id}, 'Old Guess', '{old}'), ({id}, 'New Guess', '{}')",
        common::today_str()
    ))
    .await;

    assert_eq!(c.get("/me").await.json()["guesses"], json!(["New Guess"]));
}
