use std::{collections::HashSet, sync::RwLock};

use pepeaudio_core::{GuildId, UserId};

use crate::{Access, AuthorizationError, Authorizer, BoxPortFuture, Principal};

/// Development allow-list. A grant applies to all API player operations for
/// exactly one user/guild pair.
#[derive(Debug, Default)]
pub struct AllowListAuthorizer {
    grants: RwLock<HashSet<(UserId, GuildId)>>,
}

impl AllowListAuthorizer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// # Errors
    ///
    /// Returns unavailable if the development lock is poisoned.
    pub fn grant(&self, user_id: UserId, guild_id: GuildId) -> Result<(), AuthorizationError> {
        self.grants
            .write()
            .map_err(|_| AuthorizationError::Unavailable)?
            .insert((user_id, guild_id));
        Ok(())
    }
}

impl Authorizer for AllowListAuthorizer {
    fn authorize<'a>(
        &'a self,
        principal: &'a Principal,
        guild_id: GuildId,
        _access: Access,
    ) -> BoxPortFuture<'a, Result<(), AuthorizationError>> {
        Box::pin(async move {
            let grants = self
                .grants
                .read()
                .map_err(|_| AuthorizationError::Unavailable)?;
            if grants.contains(&(principal.user_id(), guild_id)) {
                Ok(())
            } else {
                Err(AuthorizationError::Forbidden)
            }
        })
    }
}
