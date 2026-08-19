mod support;

use std::sync::Arc;

use axum::http::{HeaderMap, HeaderValue, header::COOKIE};
use pepeaudio_api::{
    Access, AuthorizationError, Authorizer as _, Principal, PrincipalAuthenticator as _,
    SESSION_COOKIE, SessionAuthenticator,
};
use pepeaudio_auth::{FixedBotPresence, SessionData, SessionGuildAuthorizer};
use pepeaudio_core::{GuildId, UserId};

use support::{
    ABSENT_GUILD, FakeSessionStore, FakeSessions, OLD_SESSION_TOKEN, PRESENT_GUILD, SESSION_TOKEN,
    fingerprint, projection,
};

fn session_headers(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        COOKIE,
        HeaderValue::from_str(&format!("{SESSION_COOKIE}={token}")).expect("session cookie"),
    );
    headers
}

#[tokio::test]
async fn requires_current_membership_and_current_bot_presence() {
    let sessions = Arc::new(FakeSessions::default());
    let user_id = UserId::new(111).expect("user");
    let session = SessionData::new(
        projection(),
        "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC".into(),
        1_800_000_000_000,
        60_000,
    )
    .expect("session");
    sessions.seed(SESSION_TOKEN, session);
    let presence = Arc::new(FixedBotPresence::new([
        GuildId::new(PRESENT_GUILD).expect("guild")
    ]));
    let authorizer = SessionGuildAuthorizer::new(sessions, presence);
    let principal = Principal::from_session(
        user_id,
        "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",
        fingerprint(SESSION_TOKEN),
    )
    .expect("principal");

    for access in [
        Access::ReadPlayer,
        Access::ControlPlayer,
        Access::SubscribeEvents,
    ] {
        assert_eq!(
            authorizer
                .authorize(
                    &principal,
                    GuildId::new(PRESENT_GUILD).expect("guild"),
                    access,
                )
                .await,
            Ok(())
        );
    }
    assert_eq!(
        authorizer
            .authorize(
                &principal,
                GuildId::new(ABSENT_GUILD).expect("guild"),
                Access::ReadPlayer,
            )
            .await,
        Err(AuthorizationError::Forbidden)
    );
    assert_eq!(
        authorizer
            .authorize(
                &principal,
                GuildId::new(444).expect("guild"),
                Access::ControlPlayer,
            )
            .await,
        Err(AuthorizationError::Forbidden)
    );
}

#[tokio::test]
async fn replacement_login_revokes_only_the_old_session_principal() {
    let sessions = Arc::new(FakeSessions::default());
    let old = SessionData::new(
        projection(),
        "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC".into(),
        1_800_000_000_000,
        60_000,
    )
    .expect("old session");
    sessions.seed(OLD_SESSION_TOKEN, old);
    let authenticator = SessionAuthenticator::new(FakeSessionStore(sessions.clone()));
    let old_principal = authenticator
        .authenticate(&session_headers(OLD_SESSION_TOKEN))
        .await
        .expect("old principal");
    let replacement = SessionData::new(
        projection(),
        "NNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNN".into(),
        1_800_000_000_001,
        60_000,
    )
    .expect("replacement session");
    sessions.seed(SESSION_TOKEN, replacement);
    let replacement_principal = authenticator
        .authenticate(&session_headers(SESSION_TOKEN))
        .await
        .expect("replacement principal");

    let authorizer = SessionGuildAuthorizer::new(
        sessions,
        Arc::new(FixedBotPresence::new([
            GuildId::new(PRESENT_GUILD).expect("guild")
        ])),
    );
    assert_eq!(
        authorizer
            .authorize(
                &old_principal,
                GuildId::new(PRESENT_GUILD).expect("guild"),
                Access::ReadPlayer,
            )
            .await,
        Err(AuthorizationError::Forbidden)
    );

    assert_eq!(
        authorizer
            .authorize(
                &replacement_principal,
                GuildId::new(PRESENT_GUILD).expect("guild"),
                Access::ReadPlayer,
            )
            .await,
        Ok(())
    );
}
