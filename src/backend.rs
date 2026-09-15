use axum_login::{AuthUser, AuthnBackend, UserId};
use bcrypt::verify;
use tokio::task;

use crate::{
    Error,
    db::{self},
    server,
};

#[derive(Clone, Copy)]
pub struct Backend;

impl AuthUser for db::User {
    type Id = i64;

    fn id(&self) -> Self::Id {
        self.id
    }

    fn session_auth_hash(&self) -> &[u8] {
        self.bcrypt.as_bytes()
    }
}

impl AuthnBackend for Backend {
    type User = db::User;
    type Credentials = server::Auth;
    type Error = Error;

    async fn authenticate(
        &self,
        creds: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        let user = db::get_user_by_username(&creds.username).await?;

        task::spawn_blocking(|| {
            Ok(user.filter(|user| verify(creds.password, &user.bcrypt).is_ok_and(|b| b)))
        })
        .await?
    }

    async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        db::get_user(*user_id).await
    }
}

pub type AuthSession = axum_login::AuthSession<Backend>;
