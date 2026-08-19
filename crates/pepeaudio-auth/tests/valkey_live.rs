use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use pepeaudio_auth::{
    AuthClock as _, AuthConfig, DiscordOAuthConfig, GuildSummary, OAuthProjection,
    OpaqueSessionRepository as _, PendingOAuth, PendingOAuthStore as _, SecretString, SessionData,
    SessionPolicy, SystemAuthClock, ValkeyAuthStore,
};
use pepeaudio_core::{GuildId, UserId};
use redis::AsyncCommands as _;
use sha2::{Digest as _, Sha256};
use url::Url;
use zeroize::Zeroizing;

const PENDING_STATE_CAPACITY: u32 = 4_096;

#[tokio::test]
#[ignore = "requires PEPEAUDIO_TEST_VALKEY_URL"]
async fn state_is_one_time_and_sessions_are_hashed_and_policy_bounded() {
    let valkey_url =
        std::env::var("PEPEAUDIO_TEST_VALKEY_URL").expect("PEPEAUDIO_TEST_VALKEY_URL must be set");
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let keyspace = format!("pepeaudio:auth-live:{}:{suffix}", std::process::id());
    let discord = DiscordOAuthConfig::new(
        "123456789",
        SecretString::new("test-client-secret-never-production"),
        Url::parse("https://audio.example.test/auth/callback").expect("URL"),
    )
    .expect("Discord config");
    let config = AuthConfig::new(discord, SessionPolicy::default(), keyspace.clone(), "/")
        .expect("auth config");
    let store = ValkeyAuthStore::connect(&valkey_url, &config)
        .await
        .expect("Valkey auth store");

    let state = "TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT";
    store
        .reserve(
            state,
            PendingOAuth {
                verifier: Zeroizing::new("VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV".into()),
                created_at_ms: SystemAuthClock.now_ms().expect("time"),
            },
        )
        .await
        .expect("reserve state");
    let pending_count = sorted_set_count(&valkey_url, format!("{keyspace}:oauth-pending-states"))
        .await
        .expect("pending count");
    assert_eq!(pending_count, 1);
    assert!(store.consume(state).await.expect("consume").is_some());
    let pending_count = sorted_set_count(&valkey_url, format!("{keyspace}:oauth-pending-states"))
        .await
        .expect("pending count after consume");
    assert_eq!(pending_count, 0);
    assert!(
        store
            .consume(state)
            .await
            .expect("replay consume")
            .is_none()
    );

    let now = SystemAuthClock.now_ms().expect("time");
    let session = SessionData::new(
        OAuthProjection {
            user_id: UserId::new(111).expect("user"),
            profile: None,
            guilds: vec![GuildSummary {
                id: GuildId::new(222).expect("guild"),
                name: "Live".into(),
                icon: None,
                owner: false,
                permissions: 0,
            }],
        },
        "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC".into(),
        now,
        60_000,
    )
    .expect("session");
    let token = store.create(session).await.expect("create session");
    assert!(store.load(&token).await.expect("load session").is_some());

    let client = redis::Client::open(valkey_url).expect("client");
    let mut connection = client.get_connection_manager().await.expect("connection");
    assert_pending_state_capacity(&store, &mut connection, &keyspace).await;
    let raw_key_exists: bool = connection
        .exists(format!("{keyspace}:session:{token}"))
        .await
        .expect("raw-key check");
    assert!(!raw_key_exists);
    let hash = URL_SAFE_NO_PAD.encode(Sha256::digest(token.as_bytes()));
    let hashed_key_exists: bool = connection
        .exists(format!("{keyspace}:session:{hash}"))
        .await
        .expect("hash-key check");
    assert!(hashed_key_exists);

    assert_legacy_session_is_revoked(&store, &mut connection, &keyspace, now).await;

    store.destroy(&token).await.expect("destroy session");
    assert!(store.load(&token).await.expect("load destroyed").is_none());
}

async fn assert_pending_state_capacity(
    store: &ValkeyAuthStore,
    connection: &mut redis::aio::ConnectionManager,
    keyspace: &str,
) {
    let pending_key = format!("{keyspace}:oauth-pending-states");
    let now_ms = SystemAuthClock.now_ms().expect("time");
    let _: u32 = redis::cmd("ZADD")
        .arg(&pending_key)
        .arg(now_ms.saturating_sub(1))
        .arg("expired-member")
        .query_async(connection)
        .await
        .expect("seed expired pending admission");
    let recovered_state = "EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE";
    store
        .reserve(
            recovered_state,
            PendingOAuth {
                verifier: Zeroizing::new("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF".into()),
                created_at_ms: now_ms,
            },
        )
        .await
        .expect("expired admission does not consume capacity");
    assert!(
        store
            .consume(recovered_state)
            .await
            .expect("consume recovered state")
            .is_some()
    );

    let members = (0..PENDING_STATE_CAPACITY)
        .map(|index| format!("seed-{index}"))
        .collect::<Vec<_>>();
    let mut command = redis::cmd("ZADD");
    command.arg(&pending_key);
    for member in &members {
        command.arg(now_ms + 60_000).arg(member);
    }
    command
        .query_async::<u32>(connection)
        .await
        .expect("seed pending admission");

    let state = "QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ";
    let rejected = store
        .reserve(
            state,
            PendingOAuth {
                verifier: Zeroizing::new("RRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRR".into()),
                created_at_ms: now_ms,
            },
        )
        .await;
    assert_eq!(
        rejected,
        Err(pepeaudio_auth::RepositoryError::CapacityExceeded)
    );
    let state_hash = URL_SAFE_NO_PAD.encode(Sha256::digest(state.as_bytes()));
    let state_exists: bool = connection
        .exists(format!("{keyspace}:oauth-state:{state_hash}"))
        .await
        .expect("capacity state lookup");
    assert!(!state_exists);
    let _: u32 = connection
        .del(pending_key)
        .await
        .expect("cleanup admission");
}

async fn sorted_set_count(valkey_url: &str, key: String) -> redis::RedisResult<u32> {
    let client = redis::Client::open(valkey_url)?;
    let mut connection = client.get_connection_manager().await?;
    redis::cmd("ZCARD")
        .arg(key)
        .query_async(&mut connection)
        .await
}

async fn assert_legacy_session_is_revoked(
    store: &ValkeyAuthStore,
    connection: &mut redis::aio::ConnectionManager,
    keyspace: &str,
    now: u64,
) {
    let token = "LLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLL";
    let hash = URL_SAFE_NO_PAD.encode(Sha256::digest(token.as_bytes()));
    let user = UserId::new(333).expect("legacy user");
    let created_at = now.checked_sub(31 * 60 * 1_000).expect("legacy time");
    let session = SessionData::new(
        OAuthProjection {
            user_id: user,
            profile: None,
            guilds: Vec::new(),
        },
        "DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD".into(),
        created_at,
        7 * 24 * 60 * 60 * 1_000,
    )
    .expect("legacy session");
    let session_key = format!("{keyspace}:session:{hash}");
    let pointer_key = format!("{keyspace}:user-current-session:{user}");
    let legacy_ttl_seconds = 7 * 24 * 60 * 60;
    let _: () = connection
        .set_ex(
            &session_key,
            serde_json::to_string(&session).expect("legacy JSON"),
            legacy_ttl_seconds,
        )
        .await
        .expect("seed legacy session");
    let _: () = connection
        .set_ex(&pointer_key, &hash, legacy_ttl_seconds)
        .await
        .expect("seed legacy pointer");

    assert!(
        store
            .load(token)
            .await
            .expect("load legacy session")
            .is_none()
    );
    let session_exists: bool = connection
        .exists(session_key)
        .await
        .expect("legacy session check");
    let pointer_exists: bool = connection
        .exists(pointer_key)
        .await
        .expect("legacy pointer check");
    assert!(!session_exists);
    assert!(!pointer_exists);
}
