use std::time::{Duration, Instant};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use tokio::sync::Mutex;
use url::Url;

use super::wire::TokenResponse;
use crate::{
    CatalogError, CatalogProvider, CatalogResult,
    http::{HttpError, HttpRequest, SharedTransport},
    secret::Secret,
};

const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const EARLY_REFRESH: Duration = Duration::from_secs(30);

struct CachedToken {
    value: Secret,
    expires_at: Instant,
}

pub(super) struct TokenManager {
    client_id: Secret,
    client_secret: Secret,
    cached: Mutex<Option<CachedToken>>,
    transport: SharedTransport,
}

impl TokenManager {
    pub(super) fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        transport: SharedTransport,
    ) -> CatalogResult<Self> {
        let client_id = client_id.into();
        let client_secret = client_secret.into();
        if client_id.trim().is_empty()
            || client_id.len() > 256
            || client_secret.trim().is_empty()
            || client_secret.len() > 512
        {
            return Err(CatalogError::InvalidCredentials(CatalogProvider::Spotify));
        }
        Ok(Self {
            client_id: Secret::new(client_id),
            client_secret: Secret::new(client_secret),
            cached: Mutex::new(None),
            transport,
        })
    }

    pub(super) async fn token(&self) -> CatalogResult<String> {
        let mut cached = self.cached.lock().await;
        if let Some(token) = cached.as_ref()
            && token
                .expires_at
                .checked_duration_since(Instant::now())
                .is_some_and(|remaining| remaining > EARLY_REFRESH)
        {
            return Ok(token.value.expose().to_owned());
        }
        let credentials = STANDARD.encode(format!(
            "{}:{}",
            self.client_id.expose(),
            self.client_secret.expose()
        ));
        let request = HttpRequest::post_form(
            Url::parse(TOKEN_URL).expect("constant token URL"),
            b"grant_type=client_credentials".to_vec(),
        )
        .with_header("authorization", format!("Basic {credentials}"));
        let response = self
            .transport
            .execute(request)
            .await
            .map_err(map_http_error)?;
        match response.status {
            200 => {}
            400 | 401 | 403 => {
                return Err(CatalogError::InvalidCredentials(CatalogProvider::Spotify));
            }
            429 => {
                return Err(CatalogError::RateLimited {
                    provider: CatalogProvider::Spotify,
                    retry_after_seconds: response.retry_after_seconds,
                });
            }
            _ => return Err(CatalogError::Transport(CatalogProvider::Spotify)),
        }
        let token: TokenResponse = serde_json::from_slice(&response.body)
            .map_err(|_| CatalogError::InvalidResponse(CatalogProvider::Spotify))?;
        if !token.token_type.eq_ignore_ascii_case("bearer")
            || token.access_token.trim().is_empty()
            || token.access_token.len() > 8_192
            || token.expires_in < 60
            || token.expires_in > 86_400
        {
            return Err(CatalogError::InvalidResponse(CatalogProvider::Spotify));
        }
        let value = token.access_token;
        *cached = Some(CachedToken {
            value: Secret::new(value.clone()),
            expires_at: Instant::now() + Duration::from_secs(token.expires_in),
        });
        Ok(value)
    }

    pub(super) async fn invalidate(&self) {
        *self.cached.lock().await = None;
    }
}

fn map_http_error(error: HttpError) -> CatalogError {
    match error {
        HttpError::Transport => CatalogError::Transport(CatalogProvider::Spotify),
        HttpError::ResponseTooLarge => CatalogError::ResponseTooLarge(CatalogProvider::Spotify),
    }
}
