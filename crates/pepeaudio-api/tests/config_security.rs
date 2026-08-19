use std::time::Duration;

use pepeaudio_api::{ApiConfig, Principal, SessionFingerprint};
use pepeaudio_core::UserId;

#[test]
fn api_configuration_rejects_wildcard_and_non_origin_values() {
    assert!(ApiConfig::new("*", Duration::from_secs(1)).is_err());
    assert!(ApiConfig::new("http://localhost:5173/path", Duration::from_secs(1)).is_err());
    assert!(ApiConfig::new("http://localhost:5173", Duration::ZERO).is_err());
}

#[test]
fn session_principal_rejects_an_obviously_weak_csrf_secret() {
    let user_id = UserId::new(20).expect("valid user ID");
    let fingerprint = SessionFingerprint::new("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFA")
        .expect("valid fingerprint");
    assert!(Principal::from_session(user_id, "too-short", fingerprint).is_err());
}

#[test]
fn session_identity_rejects_noncanonical_fingerprints_and_redacts_secrets() {
    assert!(SessionFingerprint::new("not-a-sha256-digest").is_err());
    assert!(SessionFingerprint::new("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").is_err());

    let csrf = "secret-session-csrf-value";
    let encoded_fingerprint = "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFA";
    let principal = Principal::from_session(
        UserId::new(20).expect("valid user ID"),
        csrf,
        SessionFingerprint::new(encoded_fingerprint).expect("valid fingerprint"),
    )
    .expect("valid principal");
    let debug = format!("{principal:?}");
    assert!(!debug.contains(csrf));
    assert!(!debug.contains(encoded_fingerprint));
}
