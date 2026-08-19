use std::time::Duration;

use futures_util::StreamExt as _;
use pepeaudio_core::UserId;
use reqwest::{Client, Response, StatusCode, redirect::Policy};
use serde::Deserialize;
use url::Url;
use zeroize::{Zeroize as _, Zeroizing};

use crate::{
    DiscordOAuthConfig, GuildSummary, OAuthProjection, OAuthProvider, OAuthProviderError,
    UserProfile, crypto::OAuthMaterial, ports::BoxAuthFuture,
};

const AUTHORIZE_URL: &str = "https://discord.com/oauth2/authorize";
const TOKEN_URL: &str = "https://discord.com/api/v10/oauth2/token";
const CURRENT_USER_URL: &str = "https://discord.com/api/v10/users/@me";
const CURRENT_GUILDS_URL: &str = "https://discord.com/api/v10/users/@me/guilds?limit=200";
const TOKEN_RESPONSE_LIMIT: usize = 32 * 1024;
const USER_RESPONSE_LIMIT: usize = 64 * 1024;
const GUILDS_RESPONSE_LIMIT: usize = 1024 * 1024;

/// Hardened Discord OAuth/REST client with fixed TLS endpoints and no redirects.
#[derive(Clone)]
pub struct DiscordOAuthClient {
    http: Client,
    config: DiscordOAuthConfig,
}

impl DiscordOAuthClient {
    /// # Errors
    ///
    /// Fails closed if a TLS-only, redirect-free client cannot be constructed.
    pub fn new(config: DiscordOAuthConfig) -> Result<Self, OAuthProviderError> {
        let http = Client::builder()
            .https_only(true)
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(12))
            .user_agent("PepeAudio/0.1 OAuth")
            .build()
            .map_err(|_| OAuthProviderError::Unavailable)?;
        Ok(Self { http, config })
    }

    pub(crate) fn authorization_url(
        &self,
        material: &OAuthMaterial,
    ) -> Result<Url, OAuthProviderError> {
        let mut url = Url::parse(AUTHORIZE_URL).map_err(|_| OAuthProviderError::Unavailable)?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.config.client_id)
            .append_pair("redirect_uri", self.config.redirect_uri.as_str())
            .append_pair("scope", "identify guilds")
            .append_pair("state", &material.state)
            .append_pair("code_challenge", &material.challenge)
            .append_pair("code_challenge_method", "S256");
        Ok(url)
    }

    async fn exchange(&self, code: &str, verifier: &str) -> Result<OAuthToken, OAuthProviderError> {
        let response = self
            .http
            .post(TOKEN_URL)
            .basic_auth(
                self.config.client_id.as_str(),
                Some(self.config.client_secret.expose()),
            )
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", self.config.redirect_uri.as_str()),
                ("code_verifier", verifier),
            ])
            .send()
            .await
            .map_err(|_| OAuthProviderError::Unavailable)?;
        let body = Zeroizing::new(
            checked_body(response, TOKEN_RESPONSE_LIMIT, EndpointKind::Token).await?,
        );
        let wire: TokenWire =
            serde_json::from_slice(&body).map_err(|_| OAuthProviderError::InvalidResponse)?;
        OAuthToken::try_from(wire)
    }

    async fn identity(
        &self,
        token: &OAuthToken,
    ) -> Result<(UserId, UserProfile), OAuthProviderError> {
        let response = self
            .http
            .get(CURRENT_USER_URL)
            .bearer_auth(token.access())
            .send()
            .await
            .map_err(|_| OAuthProviderError::Unavailable)?;
        let body = checked_body(response, USER_RESPONSE_LIMIT, EndpointKind::Resource).await?;
        let user: UserWire =
            serde_json::from_slice(&body).map_err(|_| OAuthProviderError::InvalidResponse)?;
        if user.bot.unwrap_or(false) {
            return Err(OAuthProviderError::InvalidResponse);
        }
        let user_id = user
            .id
            .parse()
            .map_err(|_| OAuthProviderError::InvalidResponse)?;
        let profile = UserProfile::new(user.username, user.global_name, user.avatar)
            .ok_or(OAuthProviderError::InvalidResponse)?;
        Ok((user_id, profile))
    }

    async fn guilds(&self, token: &OAuthToken) -> Result<Vec<GuildSummary>, OAuthProviderError> {
        let response = self
            .http
            .get(CURRENT_GUILDS_URL)
            .bearer_auth(token.access())
            .send()
            .await
            .map_err(|_| OAuthProviderError::Unavailable)?;
        let body = checked_body(response, GUILDS_RESPONSE_LIMIT, EndpointKind::Resource).await?;
        let guilds: Vec<GuildSummary> =
            serde_json::from_slice(&body).map_err(|_| OAuthProviderError::InvalidResponse)?;
        if guilds.len() > 200
            || guilds.iter().any(|guild| {
                guild.name.is_empty()
                    || guild.name.len() > 256
                    || guild.name.chars().any(char::is_control)
                    || guild.icon.as_ref().is_some_and(|icon| {
                        icon.len() > 128
                            || !icon
                                .bytes()
                                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                    })
            })
        {
            return Err(OAuthProviderError::InvalidResponse);
        }
        Ok(guilds)
    }
}

impl OAuthProvider for DiscordOAuthClient {
    fn exchange_projection<'a>(
        &'a self,
        code: &'a str,
        verifier: &'a str,
    ) -> BoxAuthFuture<'a, Result<OAuthProjection, OAuthProviderError>> {
        Box::pin(async move {
            let token = self.exchange(code, verifier).await?;
            let (user_id, profile) = self.identity(&token).await?;
            let guilds = self.guilds(&token).await?;
            drop(token);
            Ok(OAuthProjection {
                user_id,
                profile: Some(profile),
                guilds,
            })
        })
    }
}

async fn checked_body(
    response: Response,
    limit: usize,
    endpoint: EndpointKind,
) -> Result<Vec<u8>, OAuthProviderError> {
    let status = response.status();
    if !status.is_success() {
        return Err(status_error(status, endpoint));
    }
    if response
        .content_length()
        .is_some_and(|length| length > u64::try_from(limit).unwrap_or(u64::MAX))
    {
        return Err(OAuthProviderError::InvalidResponse);
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| OAuthProviderError::Unavailable)?;
        if body.len().saturating_add(chunk.len()) > limit {
            return Err(OAuthProviderError::InvalidResponse);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn status_error(status: StatusCode, endpoint: EndpointKind) -> OAuthProviderError {
    if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        OAuthProviderError::Unavailable
    } else if endpoint == EndpointKind::Token && status.is_client_error() {
        OAuthProviderError::Rejected
    } else {
        OAuthProviderError::InvalidResponse
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum EndpointKind {
    Token,
    Resource,
}

#[derive(Deserialize)]
struct TokenWire {
    access_token: String,
    token_type: String,
    refresh_token: Option<String>,
    expires_in: u64,
    scope: String,
}

struct OAuthToken {
    access_token: Zeroizing<String>,
    refresh_token: Option<Zeroizing<String>>,
}

impl OAuthToken {
    fn access(&self) -> &str {
        self.access_token.as_str()
    }
}

impl TryFrom<TokenWire> for OAuthToken {
    type Error = OAuthProviderError;

    fn try_from(mut wire: TokenWire) -> Result<Self, Self::Error> {
        let valid_scope = ["identify", "guilds"].into_iter().all(|required| {
            wire.scope
                .split_ascii_whitespace()
                .any(|item| item == required)
        });
        if wire.token_type != "Bearer"
            || wire.expires_in == 0
            || wire.access_token.len() < 16
            || !valid_scope
        {
            wire.access_token.zeroize();
            if let Some(refresh) = &mut wire.refresh_token {
                refresh.zeroize();
            }
            return Err(OAuthProviderError::InvalidResponse);
        }
        Ok(Self {
            access_token: Zeroizing::new(wire.access_token),
            refresh_token: wire.refresh_token.map(Zeroizing::new),
        })
    }
}

impl Drop for OAuthToken {
    fn drop(&mut self) {
        if let Some(refresh) = &mut self.refresh_token {
            refresh.zeroize();
        }
    }
}

#[derive(Deserialize)]
struct UserWire {
    id: String,
    username: String,
    global_name: Option<String>,
    avatar: Option<String>,
    bot: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::{OAuthProviderError, OAuthToken, TokenWire};

    #[test]
    fn requires_bearer_and_both_requested_scopes() {
        let valid = TokenWire {
            access_token: "a-valid-access-token-value".into(),
            token_type: "Bearer".into(),
            refresh_token: Some("a-refresh-token-value".into()),
            expires_in: 60,
            scope: "guilds identify".into(),
        };
        assert!(OAuthToken::try_from(valid).is_ok());

        let missing_scope = TokenWire {
            access_token: "a-valid-access-token-value".into(),
            token_type: "Bearer".into(),
            refresh_token: None,
            expires_in: 60,
            scope: "identify".into(),
        };
        assert_eq!(
            OAuthToken::try_from(missing_scope).err(),
            Some(OAuthProviderError::InvalidResponse)
        );
    }
}
