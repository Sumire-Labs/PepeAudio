#![allow(dead_code)]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use pepeaudio_api::{
    AuthenticationError, BoxPortFuture, SessionFingerprint, SessionRecord, SessionStore,
};
use pepeaudio_auth::{
    AuthClock, AuthConfig, BoxAuthFuture, ClockError, DiscordOAuthClient, DiscordOAuthConfig,
    FixedBotPresence, GuildSummary, OAuthProjection, OAuthProvider, OAuthProviderError,
    OpaqueSessionRepository, PendingOAuth, PendingOAuthStore, RepositoryError, SecretString,
    SessionData, SessionPolicy, UserProfile,
};
use pepeaudio_core::{GuildId, UserId};
use sha2::{Digest as _, Sha256};
use url::Url;

pub(crate) const USER_ID: u64 = 111;
pub(crate) const PRESENT_GUILD: u64 = 222;
pub(crate) const ABSENT_GUILD: u64 = 333;
pub(crate) const SESSION_TOKEN: &str = "SSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSS";
pub(crate) const OLD_SESSION_TOKEN: &str = "OOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOO";

pub(crate) fn fingerprint(token: &str) -> SessionFingerprint {
    let digest = Sha256::digest(token.as_bytes());
    SessionFingerprint::new(URL_SAFE_NO_PAD.encode(digest)).expect("canonical session fingerprint")
}

pub(crate) fn config() -> AuthConfig {
    let discord = DiscordOAuthConfig::new(
        "123456789",
        SecretString::new("test-client-secret-never-production"),
        Url::parse("https://audio.example.test/auth/callback").expect("callback URL"),
    )
    .expect("Discord config");
    AuthConfig::new(
        discord,
        SessionPolicy::default(),
        "pepeaudio:test:auth",
        "/app",
    )
    .expect("auth config")
}

pub(crate) fn projection() -> OAuthProjection {
    OAuthProjection {
        user_id: UserId::new(USER_ID).expect("user"),
        profile: Some(
            UserProfile::new(
                "pepe-listener".into(),
                Some("Pepe Listener".into()),
                Some("a_profilehash".into()),
            )
            .expect("profile"),
        ),
        guilds: vec![
            GuildSummary {
                id: GuildId::new(PRESENT_GUILD).expect("guild"),
                name: "Present".into(),
                icon: Some("abcdef012345".into()),
                owner: false,
                permissions: u64::MAX,
            },
            GuildSummary {
                id: GuildId::new(ABSENT_GUILD).expect("guild"),
                name: "No Bot".into(),
                icon: None,
                owner: true,
                permissions: 0,
            },
        ],
    }
}

#[derive(Default)]
pub(crate) struct FakeOAuth {
    pub(crate) calls: Mutex<Vec<(String, String)>>,
}

impl OAuthProvider for FakeOAuth {
    fn exchange_projection<'a>(
        &'a self,
        code: &'a str,
        verifier: &'a str,
    ) -> BoxAuthFuture<'a, Result<OAuthProjection, OAuthProviderError>> {
        Box::pin(async move {
            self.calls
                .lock()
                .expect("OAuth calls")
                .push((code.to_owned(), verifier.to_owned()));
            Ok(projection())
        })
    }
}

#[derive(Default)]
pub(crate) struct FakePending {
    values: Mutex<HashMap<String, PendingOAuth>>,
    capacity_exhausted: std::sync::atomic::AtomicBool,
}

impl FakePending {
    pub(crate) fn exhaust_capacity(&self) {
        self.capacity_exhausted
            .store(true, std::sync::atomic::Ordering::Release);
    }
}

impl PendingOAuthStore for FakePending {
    fn reserve<'a>(
        &'a self,
        state: &'a str,
        pending: PendingOAuth,
    ) -> BoxAuthFuture<'a, Result<(), RepositoryError>> {
        Box::pin(async move {
            if self
                .capacity_exhausted
                .load(std::sync::atomic::Ordering::Acquire)
            {
                return Err(RepositoryError::CapacityExceeded);
            }
            let mut values = self.values.lock().expect("pending values");
            if values.insert(state.to_owned(), pending).is_some() {
                Err(RepositoryError::Collision)
            } else {
                Ok(())
            }
        })
    }

    fn consume<'a>(
        &'a self,
        state: &'a str,
    ) -> BoxAuthFuture<'a, Result<Option<PendingOAuth>, RepositoryError>> {
        Box::pin(async move { Ok(self.values.lock().expect("pending values").remove(state)) })
    }
}

#[derive(Default)]
pub(crate) struct FakeSessions {
    values: Mutex<HashMap<String, SessionData>>,
    current: Mutex<HashMap<UserId, String>>,
}

impl FakeSessions {
    pub(crate) fn contains(&self, token: &str) -> bool {
        self.values.lock().expect("sessions").contains_key(token)
    }

    pub(crate) fn seed(&self, token: &str, session: SessionData) {
        self.current
            .lock()
            .expect("current sessions")
            .insert(session.user_id, fingerprint(token).as_str().to_owned());
        self.values
            .lock()
            .expect("sessions")
            .insert(token.to_owned(), session);
    }
}

impl OpaqueSessionRepository for FakeSessions {
    fn create(&self, session: SessionData) -> BoxAuthFuture<'_, Result<String, RepositoryError>> {
        Box::pin(async move {
            self.seed(SESSION_TOKEN, session);
            Ok(SESSION_TOKEN.to_owned())
        })
    }

    fn load<'a>(
        &'a self,
        opaque_token: &'a str,
    ) -> BoxAuthFuture<'a, Result<Option<SessionData>, RepositoryError>> {
        Box::pin(async move {
            let session = self
                .values
                .lock()
                .expect("sessions")
                .get(opaque_token)
                .cloned();
            let is_current = session.as_ref().is_some_and(|value| {
                self.current
                    .lock()
                    .expect("current sessions")
                    .get(&value.user_id)
                    .is_some_and(|current| current == fingerprint(opaque_token).as_str())
            });
            Ok(is_current.then_some(session).flatten())
        })
    }

    fn load_bound<'a>(
        &'a self,
        user_id: UserId,
        session_fingerprint: &'a str,
    ) -> BoxAuthFuture<'a, Result<Option<SessionData>, RepositoryError>> {
        Box::pin(async move {
            let is_current = self
                .current
                .lock()
                .expect("current sessions")
                .get(&user_id)
                .is_some_and(|current| current == session_fingerprint);
            if !is_current {
                return Ok(None);
            }
            Ok(self
                .values
                .lock()
                .expect("sessions")
                .iter()
                .find(|(token, session)| {
                    session.user_id == user_id && fingerprint(token).as_str() == session_fingerprint
                })
                .map(|(_, session)| session.clone()))
        })
    }

    fn destroy<'a>(
        &'a self,
        opaque_token: &'a str,
    ) -> BoxAuthFuture<'a, Result<(), RepositoryError>> {
        Box::pin(async move {
            let session = self.values.lock().expect("sessions").remove(opaque_token);
            if let Some(session) = session {
                let mut current = self.current.lock().expect("current sessions");
                if current
                    .get(&session.user_id)
                    .is_some_and(|current| current == fingerprint(opaque_token).as_str())
                {
                    current.remove(&session.user_id);
                }
            }
            Ok(())
        })
    }
}

#[derive(Clone)]
pub(crate) struct FakeSessionStore(pub(crate) Arc<FakeSessions>);

impl SessionStore for FakeSessionStore {
    fn load_session<'a>(
        &'a self,
        opaque_token: &'a str,
    ) -> BoxPortFuture<'a, Result<SessionRecord, AuthenticationError>> {
        Box::pin(async move {
            let session = self
                .0
                .load(opaque_token)
                .await
                .map_err(|_| AuthenticationError::Unavailable)?
                .ok_or(AuthenticationError::Unauthenticated)?;
            Ok(SessionRecord {
                user_id: session.user_id,
                csrf_token: Arc::from(session.csrf_token),
                session_fingerprint: fingerprint(opaque_token),
            })
        })
    }
}

#[derive(Clone, Copy)]
pub(crate) struct FixedClock(pub(crate) u64);

impl AuthClock for FixedClock {
    fn now_ms(&self) -> Result<u64, ClockError> {
        Ok(self.0)
    }
}

pub(crate) struct TestParts {
    pub(crate) service: pepeaudio_auth::AuthService,
    pub(crate) oauth: Arc<FakeOAuth>,
    pub(crate) sessions: Arc<FakeSessions>,
    pub(crate) pending: Arc<FakePending>,
}

pub(crate) fn parts() -> TestParts {
    let config = config();
    let discord = DiscordOAuthClient::new(config.discord().clone()).expect("Discord client");
    let oauth = Arc::new(FakeOAuth::default());
    let pending = Arc::new(FakePending::default());
    let sessions = Arc::new(FakeSessions::default());
    let presence = Arc::new(FixedBotPresence::new([
        GuildId::new(PRESENT_GUILD).expect("guild")
    ]));
    let service = pepeaudio_auth::AuthService::new(
        config,
        oauth.clone(),
        pending.clone(),
        sessions.clone(),
        presence,
        Arc::new(FixedClock(1_800_000_000_000)),
        Some(discord),
    );
    TestParts {
        service,
        oauth,
        sessions,
        pending,
    }
}
