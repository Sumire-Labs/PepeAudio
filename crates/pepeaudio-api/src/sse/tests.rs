use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use futures_util::{StreamExt as _, stream};
use pepeaudio_core::{GuildId, UserId};

use crate::{
    Access, ApiShutdown, AuthorizationError, Authorizer, BoxPortFuture, Principal,
    SessionFingerprint,
};

use super::{PlayerSseStream, authorization_guarded_stream, shutdown_guarded_stream};

struct SwitchAuthorizer(AtomicBool);

impl Authorizer for SwitchAuthorizer {
    fn authorize<'a>(
        &'a self,
        _principal: &'a Principal,
        _guild_id: GuildId,
        _access: Access,
    ) -> BoxPortFuture<'a, Result<(), AuthorizationError>> {
        Box::pin(async move {
            if self.0.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(AuthorizationError::Forbidden)
            }
        })
    }
}

struct CurrentSessionAuthorizer(Mutex<SessionFingerprint>);

impl Authorizer for CurrentSessionAuthorizer {
    fn authorize<'a>(
        &'a self,
        principal: &'a Principal,
        _guild_id: GuildId,
        _access: Access,
    ) -> BoxPortFuture<'a, Result<(), AuthorizationError>> {
        Box::pin(async move {
            let current = self.0.lock().expect("current session");
            if principal.session_fingerprint() == Some(&*current) {
                Ok(())
            } else {
                Err(AuthorizationError::Forbidden)
            }
        })
    }
}

fn pending_stream() -> PlayerSseStream {
    Box::pin(stream::pending())
}

fn principal() -> Principal {
    Principal::from_session(
        UserId::new(1).expect("user"),
        "test-session-csrf-token",
        SessionFingerprint::new("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFA")
            .expect("fingerprint"),
    )
    .expect("principal")
}

fn replacement_principal() -> Principal {
    Principal::from_session(
        UserId::new(1).expect("user"),
        "replacement-session-csrf-token",
        SessionFingerprint::new("RRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRA")
            .expect("fingerprint"),
    )
    .expect("principal")
}

#[tokio::test(start_paused = true)]
async fn session_expiry_closes_an_idle_stream_on_reauthorization() {
    let authorizer = Arc::new(SwitchAuthorizer(AtomicBool::new(true)));
    let mut guarded = authorization_guarded_stream(
        pending_stream(),
        authorizer.clone(),
        principal(),
        GuildId::new(2).expect("guild"),
        Duration::from_secs(10),
        Duration::from_hours(1),
    );

    assert!(
        tokio::time::timeout(Duration::from_secs(1), guarded.next())
            .await
            .is_err()
    );
    authorizer.0.store(false, Ordering::SeqCst);
    tokio::time::advance(Duration::from_secs(10)).await;
    assert!(guarded.next().await.is_none());
}

#[tokio::test(start_paused = true)]
async fn replacement_session_closes_the_old_idle_stream() {
    let old = principal();
    let replacement = replacement_principal();
    let authorizer = Arc::new(CurrentSessionAuthorizer(Mutex::new(
        old.session_fingerprint().expect("session").clone(),
    )));
    let mut guarded = authorization_guarded_stream(
        pending_stream(),
        authorizer.clone(),
        old,
        GuildId::new(2).expect("guild"),
        Duration::from_secs(10),
        Duration::from_hours(1),
    );

    *authorizer.0.lock().expect("current session") = replacement
        .session_fingerprint()
        .expect("replacement session")
        .clone();
    tokio::time::advance(Duration::from_secs(10)).await;
    assert!(guarded.next().await.is_none());
}

#[tokio::test(start_paused = true)]
async fn maximum_lifetime_forces_a_fresh_authenticated_connection() {
    let mut guarded = authorization_guarded_stream(
        pending_stream(),
        Arc::new(SwitchAuthorizer(AtomicBool::new(true))),
        principal(),
        GuildId::new(2).expect("guild"),
        Duration::from_secs(10),
        Duration::from_secs(30),
    );

    tokio::time::advance(Duration::from_secs(30)).await;
    assert!(guarded.next().await.is_none());
}

#[tokio::test(start_paused = true)]
async fn api_shutdown_closes_an_idle_stream_without_waiting_for_lease_expiry() {
    let shutdown = ApiShutdown::new();
    let mut guarded = shutdown_guarded_stream(pending_stream(), shutdown.subscribe());

    assert!(
        tokio::time::timeout(Duration::from_secs(1), guarded.next())
            .await
            .is_err()
    );
    shutdown.trigger();
    assert!(guarded.next().await.is_none());
}

#[tokio::test]
async fn stream_created_after_shutdown_is_closed_immediately() {
    let shutdown = ApiShutdown::new();
    shutdown.trigger();

    let mut guarded = shutdown_guarded_stream(pending_stream(), shutdown.subscribe());
    assert!(guarded.next().await.is_none());
}
