//! Unit tests for `Error`, the `err!` macro, and the HTTP mapping in
//! `impl IntoResponse for Error`.

use std::{io, str};

use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};
use cosmeredle::{Error, err};
use sqlx::SqlitePool;

async fn parts(e: Error) -> (StatusCode, String) {
    let res = e.into_response();
    let status = res.status();
    let bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("collect body");
    (status, String::from_utf8(bytes.to_vec()).expect("utf-8 body"))
}

const ODIUM: &str = "Odium's influence has blocked your request";

#[test]
fn err_macro_formats_into_a_string_error() {
    let e = err!("plain");
    assert_eq!(e.to_string(), "plain");
    assert!(matches!(e, Error::String(_)));

    let e = err!("{} and {}", 1, "two");
    assert_eq!(e.to_string(), "1 and two");

    let name = "Kaladin";
    assert_eq!(err!("no such character {name}").to_string(), "no such character Kaladin");
}

#[test]
fn display_messages_are_prefixed_by_kind() {
    let io_err = Error::Io(io::Error::other("disk on fire"));
    assert_eq!(io_err.to_string(), "I/O error: disk on fire");

    let sql = Error::Sql(sqlx::Error::RowNotFound);
    assert!(sql.to_string().starts_with("Database error: "), "{sql}");

    let invalid = vec![0xffu8];
    let utf8 = Error::Utf8(str::from_utf8(&invalid).unwrap_err());
    assert_eq!(utf8.to_string(), "Invalid UTF-8 encountered");

    // The two free-form variants are passed through verbatim...
    assert_eq!(Error::String("raw".into()).to_string(), "raw");
    assert_eq!(Error::User("raw".into()).to_string(), "raw");
}

#[tokio::test]
async fn user_errors_become_400_and_echo_their_message() {
    let (status, body) = parts(Error::User("pick a real character".into())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, "pick a real character");
}

#[tokio::test]
async fn user_error_message_is_reflected_to_the_client_verbatim() {
    // Worth knowing when deciding what may be put in an `Error::User`.
    let (_, body) = parts(Error::User("<script>alert(1)</script>".into())).await;
    assert_eq!(body, "<script>alert(1)</script>");
}

#[tokio::test]
async fn internal_errors_become_500_with_a_generic_message() {
    let invalid = vec![0xffu8];
    let cases = vec![
        Error::Io(io::Error::other("boom")),
        Error::String("internal detail".into()),
        Error::Sql(sqlx::Error::RowNotFound),
        Error::Utf8(str::from_utf8(&invalid).unwrap_err()),
        Error::Format(std::fmt::Error),
    ];

    for e in cases {
        let label = e.to_string();
        let (status, body) = parts(e).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{label}");
        assert_eq!(body, ODIUM, "{label}");
    }
}

#[tokio::test]
async fn internal_errors_do_not_leak_their_detail_to_the_client() {
    let (_, body) = parts(Error::String("secret: /etc/passwd".into())).await;
    assert!(!body.contains("secret"), "body leaked internals: {body}");
}

/// `sqlx::Error::RowNotFound` is what a missing row produces, and it maps to a
/// 500 rather than a 404/400.
#[tokio::test]
async fn missing_row_is_reported_as_an_internal_error() {
    let (status, _) = parts(Error::Sql(sqlx::Error::RowNotFound)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

async fn unique_violation() -> sqlx::Error {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite");
    sqlx::query("CREATE TABLE t (x TEXT PRIMARY KEY)")
        .execute(&pool)
        .await
        .expect("create");
    sqlx::query("INSERT INTO t (x) VALUES ('a')")
        .execute(&pool)
        .await
        .expect("first insert");

    sqlx::query("INSERT INTO t (x) VALUES ('a')")
        .execute(&pool)
        .await
        .expect_err("second insert must violate the primary key")
}

#[tokio::test]
async fn unique_violations_become_409() {
    let (status, body) = parts(Error::Sql(unique_violation().await)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body, "", "409 is returned with no explanatory body");
}

#[tokio::test]
async fn non_unique_database_errors_still_become_500() {
    let pool = SqlitePool::connect("sqlite::memory:").await.expect("pool");
    let e = sqlx::query("SELECT * FROM does_not_exist")
        .execute(&pool)
        .await
        .expect_err("missing table");

    let (status, body) = parts(Error::Sql(e)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body, ODIUM);
}

#[test]
fn from_impls_cover_the_question_mark_paths() {
    fn io_path() -> cosmeredle::Result<()> {
        Err(io::Error::other("x"))?;
        unreachable!()
    }
    fn sql_path() -> cosmeredle::Result<()> {
        Err(sqlx::Error::RowNotFound)?;
        unreachable!()
    }
    fn fmt_path() -> cosmeredle::Result<()> {
        Err(std::fmt::Error)?;
        unreachable!()
    }

    assert!(matches!(io_path(), Err(Error::Io(_))));
    assert!(matches!(sql_path(), Err(Error::Sql(_))));
    assert!(matches!(fmt_path(), Err(Error::Format(_))));
}
