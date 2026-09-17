use axum_login::{AuthUser, AuthnBackend, UserId};
use bcrypt::verify;
use tokio::task;

use crate::{
    Error,
    db::{self, get_user_auth_by_id},
    server,
};

#[derive(Clone, Copy)]
pub struct Backend;

#[derive(Debug, Clone)]
pub struct UserAuth {
    pub id: i64,
    pub username: String,
    pub bcrypt: String,
}

impl AuthUser for UserAuth {
    type Id = i64;

    fn id(&self) -> Self::Id {
        self.id
    }

    fn session_auth_hash(&self) -> &[u8] {
        self.bcrypt.as_bytes()
    }
}

impl AuthnBackend for Backend {
    type User = UserAuth;
    type Credentials = server::Auth;
    type Error = Error;

    async fn authenticate(
        &self,
        creds: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        let user = db::get_user_auth_by_name(&creds.username).await?;

        task::spawn_blocking(|| {
            Ok(user.filter(|user| verify(creds.password, &user.bcrypt).is_ok_and(|b| b)))
        })
        .await?
    }

    async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        get_user_auth_by_id(*user_id).await
    }
}

pub type AuthSession = axum_login::AuthSession<Backend>;
