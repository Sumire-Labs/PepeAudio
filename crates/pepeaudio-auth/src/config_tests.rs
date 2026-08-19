use std::time::Duration;

use url::Url;

use super::{AuthConfig, AuthConfigError, DiscordOAuthConfig, SecretString, SessionPolicy};

#[test]
fn rejects_non_https_callback_and_open_redirect() {
    assert!(
        DiscordOAuthConfig::new(
            "123",
            SecretString::new("a-long-enough-client-secret"),
            Url::parse("http://example.test/auth/callback").expect("URL")
        )
        .is_err()
    );

    let discord = DiscordOAuthConfig::new(
        "123",
        SecretString::new("a-long-enough-client-secret"),
        Url::parse("https://example.test/auth/callback").expect("URL"),
    )
    .expect("Discord config");
    assert!(
        AuthConfig::new(
            discord,
            SessionPolicy::default(),
            "pepeaudio:test",
            "//evil.test"
        )
        .is_err()
    );
}

#[test]
fn bounds_login_time_membership_to_thirty_minutes() {
    let maximum = Duration::from_mins(30);
    assert_eq!(
        SessionPolicy::new(maximum, maximum, Duration::from_mins(5))
            .expect("thirty-minute policy")
            .absolute_ttl(),
        maximum
    );
    assert_eq!(
        SessionPolicy::new(
            maximum + Duration::from_secs(1),
            maximum,
            Duration::from_mins(5)
        ),
        Err(AuthConfigError::AbsoluteLifetimeTooLong)
    );
    assert_eq!(SessionPolicy::default().absolute_ttl(), maximum);
}
