//! Tests for `backend::Backend`, the `axum-login` authentication backend, and
//! the `AuthUser` impl that ties a session to a password hash.

mod common;

use axum_login::{AuthUser, AuthnBackend};
use bcrypt::hash;
use cosmeredle::{
    backend::{Backend, UserAuth},
    db::{get_user_auth_by_name, insert_user},
    server::Auth,
};

/// Cost 4 keeps the tests fast; production signup uses 12.
fn bcrypt(password: &str) -> String {
    hash(password, 4).expect("hash")
}

fn creds(username: &str, password: &str) -> Auth {
    Auth {
        username: username.to_string(),
        password: password.to_string(),
    }
}

async fn user(username: &str, password: &str) -> i64 {
    common::setup().await;
    insert_user(username.into(), bcrypt(password)).await.expect("insert");
    get_user_auth_by_name(username).await.expect("query").expect("user").id
}

#[tokio::test]
async fn correct_credentials_authenticate() {
    user("auth-ok", "correct horse").await;

    let found = Backend
        .authenticate(creds("auth-ok", "correct horse"))
        .await
        .expect("no error")
        .expect("authenticated");

    assert_eq!(found.username, "auth-ok");
}

#[tokio::test]
async fn a_wrong_password_authenticates_to_none() {
    user("auth-wrong", "correct horse").await;

    let found = Backend
        .authenticate(creds("auth-wrong", "wrong horse"))
        .await
        .expect("no error");
    assert!(found.is_none());
}

#[tokio::test]
async fn an_unknown_user_authenticates_to_none() {
    common::setup().await;

    let found = Backend
        .authenticate(creds("auth-nobody", "anything"))
        .await
        .expect("a missing user is not an error");
    assert!(found.is_none());
}

#[tokio::test]
async fn passwords_are_compared_case_sensitively_and_without_trimming() {
    user("auth-exact", "Secret").await;

    for password in ["secret", "SECRET", " Secret", "Secret "] {
        assert!(
            Backend
                .authenticate(creds("auth-exact", password))
                .await
                .expect("no error")
                .is_none(),
            "{password:?} should not authenticate"
        );
    }
}

/// A row whose `bcrypt` column is not a valid hash must fail closed rather
/// than erroring or matching.
#[tokio::test]
async fn a_corrupt_stored_hash_authenticates_to_none() {
    common::setup().await;
    insert_user("auth-corrupt".into(), "not-a-bcrypt-hash".into())
        .await
        .expect("insert");

    for password in ["not-a-bcrypt-hash", "", "anything"] {
        assert!(
            Backend
                .authenticate(creds("auth-corrupt", password))
                .await
                .expect("no error")
                .is_none(),
            "{password:?} should not authenticate against a corrupt hash"
        );
    }
}

#[tokio::test]
async fn an_empty_password_only_matches_a_hash_of_the_empty_password() {
    user("auth-empty", "").await;

    assert!(
        Backend
            .authenticate(creds("auth-empty", ""))
            .await
            .expect("no error")
            .is_some()
    );
    assert!(
        Backend
            .authenticate(creds("auth-empty", "x"))
            .await
            .expect("no error")
            .is_none()
    );
}

#[tokio::test]
async fn get_user_loads_by_id() {
    let id = user("auth-by-id", "pw").await;

    let found = Backend.get_user(&id).await.expect("no error").expect("user");
    assert_eq!(found.id, id);
    assert_eq!(found.username, "auth-by-id");

    assert!(Backend.get_user(&999_999).await.expect("no error").is_none());
}

#[test]
fn auth_user_exposes_its_id_and_hash() {
    let u = UserAuth {
        id: 7,
        username: "session".into(),
        bcrypt: "$2b$04$hash".into(),
    };

    assert_eq!(u.id(), 7);
    // Sessions are bound to the password hash, so changing a password logs
    // every existing session out.
    assert_eq!(u.session_auth_hash(), b"$2b$04$hash");
}
