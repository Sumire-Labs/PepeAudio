use std::{sync::Arc, time::Duration};

use url::Url;

use crate::{
    AuthClock, AuthConfig, BotPresencePort, DiscordOAuthClient, GuildView, OAuthProvider,
    OpaqueSessionRepository, PendingOAuth, PendingOAuthStore, RepositoryError, SessionData,
    SessionView,
    crypto::{constant_time_eq, oauth_material, random_token},
    ports::{ClockError, OAuthProviderError},
};

const MATERIAL_ATTEMPTS: usize = 3;

#[derive(Clone)]
pub struct AuthService {
    pub(crate) config: Arc<AuthConfig>,
    provider: Arc<dyn OAuthProvider>,
    pending: Arc<dyn PendingOAuthStore>,
    pub(crate) sessions: Arc<dyn OpaqueSessionRepository>,
    pub(crate) bot_presence: Arc<dyn BotPresencePort>,
    clock: Arc<dyn AuthClock>,
    discord_urls: Option<DiscordOAuthClient>,
}

impl AuthService {
    /// Creates an injectable service. `discord_urls` is required for `/login`;
    /// tests may omit it and inject an explicit authorization URL factory.
    #[must_use]
    pub fn new(
        config: AuthConfig,
        provider: Arc<dyn OAuthProvider>,
        pending: Arc<dyn PendingOAuthStore>,
        sessions: Arc<dyn OpaqueSessionRepository>,
        bot_presence: Arc<dyn BotPresencePort>,
        clock: Arc<dyn AuthClock>,
        discord_urls: Option<DiscordOAuthClient>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            provider,
            pending,
            sessions,
            bot_presence,
            clock,
            discord_urls,
        }
    }

    #[must_use]
    pub fn with_discord_client(
        config: AuthConfig,
        discord: DiscordOAuthClient,
        pending: Arc<dyn PendingOAuthStore>,
        sessions: Arc<dyn OpaqueSessionRepository>,
        bot_presence: Arc<dyn BotPresencePort>,
        clock: Arc<dyn AuthClock>,
    ) -> Self {
        Self::new(
            config,
            Arc::new(discord.clone()),
            pending,
            sessions,
            bot_presence,
            clock,
            Some(discord),
        )
    }

    pub(crate) async fn begin_login(&self) -> Result<LoginStart, AuthServiceError> {
        let now_ms = self.clock.now_ms()?;
        for _ in 0..MATERIAL_ATTEMPTS {
            let material = oauth_material().map_err(|_| AuthServiceError::Unavailable)?;
            let authorization_url = self
                .discord_urls
                .as_ref()
                .ok_or(AuthServiceError::Unavailable)?
                .authorization_url(&material)?;
            let pending = PendingOAuth {
                verifier: material.verifier.clone(),
                created_at_ms: now_ms,
            };
            match self.pending.reserve(&material.state, pending).await {
                Ok(()) => {
                    return Ok(LoginStart {
                        authorization_url,
                        state: material.state,
                    });
                }
                Err(RepositoryError::Collision) => {}
                Err(RepositoryError::CapacityExceeded) => {
                    return Err(AuthServiceError::CapacityExceeded);
                }
                Err(error) => return Err(error.into()),
            }
        }
        Err(AuthServiceError::Unavailable)
    }

    pub(crate) async fn complete_callback(
        &self,
        code: &str,
        returned_state: &str,
        cookie_state: &str,
    ) -> Result<String, AuthServiceError> {
        validate_code(code)?;
        if returned_state.len() != cookie_state.len()
            || !constant_time_eq(returned_state, cookie_state)
        {
            return Err(AuthServiceError::InvalidCallback);
        }
        let pending = self
            .pending
            .consume(returned_state)
            .await?
            .ok_or(AuthServiceError::InvalidCallback)?;
        let now_ms = self.clock.now_ms()?;
        let state_ttl_ms = duration_ms(self.config.session.oauth_state_ttl)?;
        if pending.created_at_ms > now_ms
            || now_ms.saturating_sub(pending.created_at_ms) > state_ttl_ms
        {
            return Err(AuthServiceError::InvalidCallback);
        }
        let projection = self
            .provider
            .exchange_projection(code, pending.verifier.as_str())
            .await?;
        if projection.guilds.len() > 200 {
            return Err(AuthServiceError::InvalidCallback);
        }
        let csrf_token = random_token().map_err(|_| AuthServiceError::Unavailable)?;
        let absolute_ttl_ms = duration_ms(self.config.session.absolute_ttl)?;
        let session = SessionData::new(projection, csrf_token, now_ms, absolute_ttl_ms)
            .ok_or(AuthServiceError::Unavailable)?;
        self.sessions.create(session).await.map_err(Into::into)
    }

    pub(crate) async fn consume_denied_callback(&self, returned_state: &str, cookie_state: &str) {
        if returned_state.len() == cookie_state.len()
            && constant_time_eq(returned_state, cookie_state)
        {
            let _ = self.pending.consume(returned_state).await;
        }
    }

    pub(crate) async fn load_session(&self, token: &str) -> Result<SessionData, AuthServiceError> {
        self.sessions
            .load(token)
            .await?
            .ok_or(AuthServiceError::Unauthenticated)
    }

    pub(crate) async fn logout(&self, token: &str) -> Result<(), AuthServiceError> {
        self.sessions.destroy(token).await.map_err(Into::into)
    }

    pub(crate) async fn session_view(&self, token: &str) -> Result<SessionView, AuthServiceError> {
        Ok(SessionView::from(&self.load_session(token).await?))
    }

    pub(crate) async fn guild_views(
        &self,
        token: &str,
    ) -> Result<Vec<GuildView>, AuthServiceError> {
        let session = self.load_session(token).await?;
        let mut views = Vec::with_capacity(session.guilds.len());
        for guild in session.guilds {
            let bot_present = self.bot_presence.is_present(guild.id).await?;
            views.push(GuildView { guild, bot_present });
        }
        Ok(views)
    }
}

pub(crate) struct LoginStart {
    pub authorization_url: Url,
    pub state: String,
}

fn validate_code(code: &str) -> Result<(), AuthServiceError> {
    if code.is_empty() || code.len() > 1024 || code.chars().any(char::is_control) {
        Err(AuthServiceError::InvalidCallback)
    } else {
        Ok(())
    }
}

fn duration_ms(duration: Duration) -> Result<u64, AuthServiceError> {
    u64::try_from(duration.as_millis())
        .ok()
        .filter(|value| *value != 0)
        .ok_or(AuthServiceError::Unavailable)
}

#[derive(Clone, Copy, Debug, Eq, thiserror::Error, PartialEq)]
pub(crate) enum AuthServiceError {
    #[error("authentication required")]
    Unauthenticated,
    #[error("invalid OAuth callback")]
    InvalidCallback,
    #[error("authentication service unavailable")]
    Unavailable,
    #[error("authentication admission capacity is exhausted")]
    CapacityExceeded,
}

impl From<RepositoryError> for AuthServiceError {
    fn from(value: RepositoryError) -> Self {
        match value {
            RepositoryError::InvalidToken => Self::Unauthenticated,
            RepositoryError::Collision
            | RepositoryError::Unavailable
            | RepositoryError::Corrupt => Self::Unavailable,
            RepositoryError::CapacityExceeded => Self::CapacityExceeded,
        }
    }
}

impl From<OAuthProviderError> for AuthServiceError {
    fn from(value: OAuthProviderError) -> Self {
        match value {
            OAuthProviderError::Rejected | OAuthProviderError::InvalidResponse => {
                Self::InvalidCallback
            }
            OAuthProviderError::Unavailable => Self::Unavailable,
        }
    }
}

impl From<ClockError> for AuthServiceError {
    fn from(_: ClockError) -> Self {
        Self::Unavailable
    }
}

impl From<crate::BotPresenceError> for AuthServiceError {
    fn from(_: crate::BotPresenceError) -> Self {
        Self::Unavailable
    }
}
