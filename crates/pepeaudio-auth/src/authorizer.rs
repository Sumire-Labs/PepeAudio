use std::sync::Arc;

use pepeaudio_api::{Access, AuthorizationError, Authorizer, BoxPortFuture, Principal};
use pepeaudio_core::GuildId;

use crate::{BotPresencePort, OpaqueSessionRepository};

/// Production guild authorizer over a single current session per Discord user.
///
/// Each principal carries only a SHA-256 fingerprint of the opaque cookie, not
/// the cookie itself. This adapter requires that exact fingerprint to still own
/// the Valkey `user-current-session` pointer on every check. A replacement
/// login, logout, or expiry therefore revokes requests and existing SSE streams
/// created by the old session.
///
/// Read, event subscription, and player control require both OAuth-time guild
/// membership and current bot presence. They intentionally do not require
/// `MANAGE_GUILD`: the Bot/player owner must still enforce voice-channel and DJ
/// policy at command application time. Configuration administration is not yet
/// granted by this policy.
#[derive(Clone)]
pub struct SessionGuildAuthorizer {
    sessions: Arc<dyn OpaqueSessionRepository>,
    bot_presence: Arc<dyn BotPresencePort>,
}

impl SessionGuildAuthorizer {
    #[must_use]
    pub fn new(
        sessions: Arc<dyn OpaqueSessionRepository>,
        bot_presence: Arc<dyn BotPresencePort>,
    ) -> Self {
        Self {
            sessions,
            bot_presence,
        }
    }
}

impl Authorizer for SessionGuildAuthorizer {
    fn authorize<'a>(
        &'a self,
        principal: &'a Principal,
        guild_id: GuildId,
        _access: Access,
    ) -> BoxPortFuture<'a, Result<(), AuthorizationError>> {
        Box::pin(async move {
            let session_fingerprint = principal
                .session_fingerprint()
                .ok_or(AuthorizationError::Forbidden)?;
            let session = self
                .sessions
                .load_bound(principal.user_id(), session_fingerprint.as_str())
                .await
                .map_err(|_| AuthorizationError::Unavailable)?
                .ok_or(AuthorizationError::Forbidden)?;
            if !session.has_guild(guild_id) {
                return Err(AuthorizationError::Forbidden);
            }
            let bot_present = self
                .bot_presence
                .is_present(guild_id)
                .await
                .map_err(|_| AuthorizationError::Unavailable)?;
            if bot_present {
                Ok(())
            } else {
                Err(AuthorizationError::Forbidden)
            }
        })
    }
}
