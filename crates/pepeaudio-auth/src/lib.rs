//! Production-oriented Discord `OAuth2` and opaque web sessions for `PepeAudio`.
//!
//! The authorization-code callback uses a one-time Valkey state bound to an
//! `HttpOnly` `__Host-` cookie and PKCE `S256`. Discord access and refresh tokens
//! exist only long enough to fetch `/users/@me` and `/users/@me/guilds`; only a
//! bounded identity/membership projection enters the server-side session.

#![forbid(unsafe_code)]

mod authorizer;
mod config;
mod cookie;
mod crypto;
mod discord;
mod handlers;
mod model;
mod ports;
mod service;
mod valkey;

pub use authorizer::SessionGuildAuthorizer;
pub use config::{AuthConfig, AuthConfigError, DiscordOAuthConfig, SecretString, SessionPolicy};
pub use discord::DiscordOAuthClient;
pub use handlers::build_auth_router;
pub use model::{
    GuildSummary, GuildView, OAuthProjection, PendingOAuth, SessionData, SessionView, UserProfile,
};
pub use ports::{
    AuthClock, BotPresenceError, BotPresencePort, BoxAuthFuture, ClockError, FixedBotPresence,
    OAuthProvider, OAuthProviderError, OpaqueSessionRepository, PendingOAuthStore, RepositoryError,
    SystemAuthClock,
};
pub use service::AuthService;
pub use valkey::ValkeyAuthStore;
