use pepeaudio_core::{GuildId, UserId};
use serde_json::json;

use super::{GuildSummary, SessionData, SessionView, UserProfile};

#[test]
fn snowflakes_and_permissions_are_decimal_strings() {
    let guild = GuildSummary {
        id: GuildId::new(9_007_199_254_740_993).expect("guild"),
        name: "Precision".into(),
        icon: None,
        owner: false,
        permissions: u64::MAX,
    };
    let encoded = serde_json::to_string(&guild).expect("JSON");
    assert!(encoded.contains(r#""id":"9007199254740993""#));
    assert!(encoded.contains(r#""permissions":"18446744073709551615""#));
}

#[test]
fn profile_validation_rejects_control_text_and_untrusted_avatar_hashes() {
    assert!(
        UserProfile::new(
            "pepe-listener".into(),
            Some("Pepe Listener".into()),
            Some("a_safehash".into())
        )
        .is_some()
    );
    assert!(UserProfile::new("bad\nname".into(), None, None).is_none());
    assert!(UserProfile::new("listener".into(), None, Some("../../avatar".into())).is_none());
}

#[test]
fn sessions_created_before_profile_projection_still_decode() {
    let encoded = json!({
        "schema_version": 1,
        "user_id": "111",
        "csrf_token": "A".repeat(43),
        "guilds": [],
        "created_at_ms": 1,
        "expires_at_ms": 3,
        "last_seen_at_ms": 2
    });
    let session: SessionData = serde_json::from_value(encoded).expect("legacy session");
    assert!(session.profile.is_none());
    assert!(session.is_valid_shape());
}

#[test]
fn session_view_debug_redacts_profile_and_csrf_values() {
    let view = SessionView {
        user_id: UserId::new(111).expect("user"),
        username: Some("private-user".into()),
        display_name: Some("Private Name".into()),
        avatar: Some("private_avatar_hash".into()),
        csrf_token: "private-csrf-token".into(),
        created_at_ms: 1,
        expires_at_ms: 2,
    };
    let debug = format!("{view:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("private-user"));
    assert!(!debug.contains("Private Name"));
    assert!(!debug.contains("private_avatar_hash"));
    assert!(!debug.contains("private-csrf-token"));
}
