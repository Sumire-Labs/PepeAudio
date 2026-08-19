mod support;

use std::{collections::HashMap, net::IpAddr, time::Duration};

use pepeaudio_media::{UrlGuard, UrlPolicyError, is_forbidden_ip};
use support::FakeDns;

fn guard() -> UrlGuard {
    UrlGuard::new(128, Duration::from_secs(1))
}

#[tokio::test]
async fn permits_public_http_and_https_targets() {
    let resolver = FakeDns::public();

    let target = guard()
        .approve("https://media.example/audio", &resolver)
        .await
        .expect("public host");

    assert_eq!(target.url().as_str(), "https://media.example/audio");
    assert_eq!(target.socket_addrs()[0].port(), 443);
}

#[tokio::test]
async fn rejects_unsafe_url_syntax_before_dns() {
    let resolver = FakeDns::public();
    let cases = [
        ("file:///etc/passwd", UrlPolicyError::UnsupportedScheme),
        ("https://user@example.com/a", UrlPolicyError::UserInfo),
        ("https://@example.com/a", UrlPolicyError::UserInfo),
        ("https://example.com/a#frag", UrlPolicyError::Fragment),
        ("https://", UrlPolicyError::Malformed),
    ];

    for (url, expected) in cases {
        let error = guard().approve(url, &resolver).await.expect_err(url);
        assert_eq!(
            std::mem::discriminant(&error),
            std::mem::discriminant(&expected),
            "{url}"
        );
    }
    assert_eq!(resolver.call_count(), 0);
}

#[tokio::test]
async fn rejects_overlong_urls() {
    let resolver = FakeDns::public();
    let error = UrlGuard::new(20, Duration::from_secs(1))
        .approve("https://example.com/long", &resolver)
        .await
        .expect_err("length bound");
    assert!(matches!(error, UrlPolicyError::TooLong { max_bytes: 20 }));
}

#[tokio::test]
async fn rejects_entire_dns_answer_when_one_address_is_private() {
    let answers = HashMap::from([(
        "media.example".to_owned(),
        vec![
            IpAddr::from([93, 184, 216, 34]),
            IpAddr::from([10, 0, 0, 1]),
        ],
    )]);
    let resolver = FakeDns::public().with_answers(answers);

    let error = guard()
        .approve("https://media.example/audio", &resolver)
        .await
        .expect_err("mixed answer must fail closed");

    assert!(matches!(error, UrlPolicyError::ForbiddenAddress));
}

#[tokio::test]
async fn rejects_excessive_dns_answer_sets() {
    let many = (1..=33).map(|last| IpAddr::from([8, 8, 8, last])).collect();
    let resolver = FakeDns::new(many);

    let error = guard()
        .approve("https://media.example/audio", &resolver)
        .await
        .expect_err("bounded DNS answer");

    assert!(matches!(error, UrlPolicyError::TooManyDnsAnswers));
}

#[test]
fn rejects_required_ipv4_and_ipv6_network_classes() {
    let forbidden = [
        "0.0.0.0",
        "127.0.0.1",
        "10.1.2.3",
        "169.254.1.1",
        "224.0.0.1",
        "100.64.0.1",
        "198.18.0.1",
        "::",
        "::1",
        "fc00::1",
        "fe80::1",
        "ff02::1",
        "::ffff:127.0.0.1",
        "::127.0.0.1",
        "64:ff9b::127.0.0.1",
        "2002:7f00:1::",
        "2001:db8::1",
    ];
    for address in forbidden {
        assert!(
            is_forbidden_ip(address.parse().expect("test address")),
            "{address}"
        );
    }
    assert!(!is_forbidden_ip("8.8.8.8".parse().expect("IPv4")));
    assert!(!is_forbidden_ip(
        "2606:4700:4700::1111".parse().expect("IPv6")
    ));
}
