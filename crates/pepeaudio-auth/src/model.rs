use std::{collections::HashSet, fmt};

use pepeaudio_core::{GuildId, UserId};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// Discord guild membership projected into a server-side session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GuildSummary {
    /// Guild snowflake, serialized as a decimal string.
    pub id: GuildId,
    pub name: String,
    /// Optional Discord icon hash, not a URL.
    pub icon: Option<String>,
    pub owner: bool,
    /// Guild-level permission bitfield, serialized as a decimal string.
    #[serde(with = "decimal_u64")]
    pub permissions: u64,
}

/// Identity and membership obtained while an OAuth token is held in memory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthProjection {
    pub user_id: UserId,
    /// Browser-safe identity projected from Discord's `identify` scope.
    pub profile: Option<UserProfile>,
    /// At most 200 partial guilds from `/users/@me/guilds`.
    pub guilds: Vec<GuildSummary>,
}

/// Browser-safe Discord identity retained without an OAuth access token.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UserProfile {
    /// Current Discord username without a leading `@`.
    pub username: String,
    /// Global display name, falling back to the username.
    pub display_name: String,
    /// Optional Discord avatar hash, not a URL.
    pub avatar: Option<String>,
}

impl UserProfile {
    #[must_use]
    pub fn new(
        username: String,
        global_name: Option<String>,
        avatar: Option<String>,
    ) -> Option<Self> {
        let display_name = global_name.unwrap_or_else(|| username.clone());
        let profile = Self {
            username,
            display_name,
            avatar,
        };
        profile.is_valid_shape().then_some(profile)
    }

    fn is_valid_shape(&self) -> bool {
        valid_profile_text(&self.username)
            && valid_profile_text(&self.display_name)
            && self
                .avatar
                .as_ref()
                .is_none_or(|avatar| valid_asset_hash(avatar))
    }
}

/// One pending, one-time OAuth transaction stored in Valkey.
pub struct PendingOAuth {
    /// PKCE verifier; zeroized after callback processing.
    pub verifier: Zeroizing<String>,
    /// Creation timestamp used for audit-safe expiry validation.
    pub created_at_ms: u64,
}

/// Full session data held only on the server.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SessionData {
    pub(crate) schema_version: u8,
    pub user_id: UserId,
    /// Optional for sessions created before profile projection was introduced.
    #[serde(default)]
    pub profile: Option<UserProfile>,
    /// Synchronizer token returned by `/auth/session`, never by a cookie.
    pub csrf_token: String,
    /// OAuth-time guild membership and permission projection.
    pub guilds: Vec<GuildSummary>,
    pub created_at_ms: u64,
    /// Hard expiry in Unix milliseconds.
    pub expires_at_ms: u64,
    /// Most recent successful server-side lookup.
    pub last_seen_at_ms: u64,
}

impl fmt::Debug for SessionData {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionData")
            .field("schema_version", &self.schema_version)
            .field("user_id", &self.user_id)
            .field("profile_present", &self.profile.is_some())
            .field("csrf_token", &"[REDACTED]")
            .field("guild_count", &self.guilds.len())
            .field("created_at_ms", &self.created_at_ms)
            .field("expires_at_ms", &self.expires_at_ms)
            .field("last_seen_at_ms", &self.last_seen_at_ms)
            .finish()
    }
}

impl SessionData {
    pub(crate) const SCHEMA_VERSION: u8 = 1;

    /// Returns `None` for an invalid CSRF token, duplicate/oversized guild
    /// projection, zero lifetime, or hard-expiry timestamp overflow.
    #[must_use]
    pub fn new(
        projection: OAuthProjection,
        csrf_token: String,
        now_ms: u64,
        absolute_ttl_ms: u64,
    ) -> Option<Self> {
        let expires_at_ms = now_ms.checked_add(absolute_ttl_ms)?;
        let session = Self {
            schema_version: Self::SCHEMA_VERSION,
            user_id: projection.user_id,
            profile: projection.profile,
            csrf_token,
            guilds: projection.guilds,
            created_at_ms: now_ms,
            expires_at_ms,
            last_seen_at_ms: now_ms,
        };
        session.is_valid_shape().then_some(session)
    }

    #[must_use]
    pub fn has_guild(&self, guild_id: GuildId) -> bool {
        self.guilds.iter().any(|guild| guild.id == guild_id)
    }

    pub(crate) fn is_valid_shape(&self) -> bool {
        let mut ids = HashSet::with_capacity(self.guilds.len());
        self.schema_version == Self::SCHEMA_VERSION
            && self.csrf_token.len() == 43
            && is_base64url(&self.csrf_token)
            && self.guilds.len() <= 200
            && self
                .profile
                .as_ref()
                .is_none_or(UserProfile::is_valid_shape)
            && self.guilds.iter().all(|guild| {
                ids.insert(guild.id)
                    && !guild.name.is_empty()
                    && guild.name.len() <= 256
                    && !guild.name.chars().any(char::is_control)
                    && guild
                        .icon
                        .as_ref()
                        .is_none_or(|icon| valid_asset_hash(icon))
            })
            && self.created_at_ms <= self.last_seen_at_ms
            && self.last_seen_at_ms < self.expires_at_ms
    }
}

fn is_base64url(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

/// Browser-safe current-session response.
#[derive(Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    /// Decimal Discord user snowflake.
    pub user_id: UserId,
    pub username: Option<String>,
    pub display_name: Option<String>,
    /// Discord avatar hash. The browser constructs the fixed CDN URL.
    pub avatar: Option<String>,
    /// Session-bound CSRF synchronizer token for mutation headers.
    pub csrf_token: String,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
}

impl fmt::Debug for SessionView {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionView")
            .field("user_id", &self.user_id)
            .field("username", &self.username.as_ref().map(|_| "[REDACTED]"))
            .field(
                "display_name",
                &self.display_name.as_ref().map(|_| "[REDACTED]"),
            )
            .field("avatar", &self.avatar.as_ref().map(|_| "[REDACTED]"))
            .field("csrf_token", &"[REDACTED]")
            .field("created_at_ms", &self.created_at_ms)
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

impl From<&SessionData> for SessionView {
    fn from(value: &SessionData) -> Self {
        Self {
            user_id: value.user_id,
            username: value
                .profile
                .as_ref()
                .map(|profile| profile.username.clone()),
            display_name: value
                .profile
                .as_ref()
                .map(|profile| profile.display_name.clone()),
            avatar: value
                .profile
                .as_ref()
                .and_then(|profile| profile.avatar.clone()),
            csrf_token: value.csrf_token.clone(),
            created_at_ms: value.created_at_ms,
            expires_at_ms: value.expires_at_ms,
        }
    }
}

fn valid_profile_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}

fn valid_asset_hash(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildView {
    #[serde(flatten)]
    pub guild: GuildSummary,
    pub bot_present: bool,
}

mod decimal_u64 {
    use serde::{Deserialize, Deserializer, Serializer, de::Error as _};

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub(super) fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(value)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
