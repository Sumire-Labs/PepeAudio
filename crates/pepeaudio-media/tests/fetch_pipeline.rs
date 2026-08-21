mod support;

use std::{collections::HashMap, net::IpAddr, time::Duration};

use bytes::Bytes;
use futures_util::stream;
use pepeaudio_media::{
    BodyError, DiscordAttachment, FetchError, FetchLimits, HttpResponse, MediaFetcher,
    MediaRequest, MediaSourceKind,
};
use support::{FakeDns, FakeHttp, TestRoot, download_store};

fn limits(maximum: u64) -> FetchLimits {
    FetchLimits {
        max_download_bytes: maximum,
        ..FetchLimits::default()
    }
}

fn response(
    status: u16,
    location: Option<&str>,
    length: Option<u64>,
    chunks: Vec<Result<&'static [u8], BodyError>>,
) -> HttpResponse {
    HttpResponse::new(
        status,
        location.map(str::to_owned),
        length,
        Some("audio/not-trusted".to_owned()),
        Box::pin(stream::iter(
            chunks
                .into_iter()
                .map(|chunk| chunk.map(Bytes::from_static)),
        )),
    )
}

#[tokio::test]
async fn direct_url_follows_validated_redirect_and_saves_extensionless_object() {
    let root = TestRoot::new("redirect");
    let http = FakeHttp::new([
        response(302, Some("https://cdn.example/song.fake"), Some(0), vec![]),
        response(200, None, Some(6), vec![Ok(b"media!")]),
    ]);
    let calls = http.clone();
    let fetcher = MediaFetcher::new(
        FakeDns::public(),
        http,
        download_store(&root).await,
        limits(32),
    )
    .expect("fetcher");

    let downloaded = fetcher
        .fetch(MediaRequest::DirectUrl {
            url: "https://origin.example/start".to_owned(),
        })
        .await
        .expect("download");

    assert_eq!(downloaded.size_bytes, 6);
    assert_eq!(downloaded.source_kind, MediaSourceKind::DirectUrl);
    assert_eq!(downloaded.path.extension(), None);
    assert_eq!(
        tokio::fs::read(&downloaded.path).await.expect("file"),
        b"media!"
    );
    assert_eq!(
        calls.calls(),
        [
            "https://origin.example/start",
            "https://cdn.example/song.fake"
        ]
    );
    assert_eq!(calls.open_range_calls(), [false, false]);
}

#[tokio::test]
async fn discord_attachment_uses_the_identical_network_path() {
    let root = TestRoot::new("attachment");
    let http = FakeHttp::new([response(200, None, Some(4), vec![Ok(b"data")])]);
    let calls = http.clone();
    let fetcher = MediaFetcher::new(
        FakeDns::public(),
        http,
        download_store(&root).await,
        limits(32),
    )
    .expect("fetcher");

    let downloaded = fetcher
        .fetch(MediaRequest::DiscordAttachment(DiscordAttachment {
            url: "https://cdn.discord.example/evil.exe".to_owned(),
            filename: "evil.exe".to_owned(),
            content_type: Some("application/x-msdownload".to_owned()),
            declared_size_bytes: Some(4),
        }))
        .await
        .expect("attachment");

    assert_eq!(downloaded.source_kind, MediaSourceKind::DiscordAttachment);
    assert_eq!(downloaded.path.extension(), None);
    assert_eq!(calls.calls(), ["https://cdn.discord.example/evil.exe"]);
    assert_eq!(calls.open_range_calls(), [false]);
}

#[tokio::test]
async fn redirect_to_private_dns_is_rejected_before_second_request() {
    let root = TestRoot::new("private-redirect");
    let answers = HashMap::from([(
        "private.example".to_owned(),
        vec![IpAddr::from([192, 168, 1, 2])],
    )]);
    let http = FakeHttp::new([response(
        302,
        Some("http://private.example/media"),
        None,
        vec![],
    )]);
    let calls = http.clone();
    let fetcher = MediaFetcher::new(
        FakeDns::public().with_answers(answers),
        http,
        download_store(&root).await,
        limits(32),
    )
    .expect("fetcher");

    let error = fetcher
        .fetch(MediaRequest::DirectUrl {
            url: "http://public.example/start".to_owned(),
        })
        .await
        .expect_err("private redirect");

    assert!(matches!(error, FetchError::Url(_)));
    assert_eq!(calls.calls(), ["http://public.example/start"]);
}

#[tokio::test]
async fn secure_redirect_cannot_downgrade_to_http() {
    let root = TestRoot::new("downgrade");
    let http = FakeHttp::new([response(
        302,
        Some("http://cdn.example/media"),
        None,
        vec![],
    )]);
    let calls = http.clone();
    let fetcher = MediaFetcher::new(
        FakeDns::public(),
        http,
        download_store(&root).await,
        limits(32),
    )
    .expect("fetcher");

    let error = fetcher
        .fetch(MediaRequest::DirectUrl {
            url: "https://origin.example/start".to_owned(),
        })
        .await
        .expect_err("downgrade");

    assert!(matches!(error, FetchError::Url(_)));
    assert_eq!(calls.calls(), ["https://origin.example/start"]);
}

#[tokio::test]
async fn content_length_is_rejected_before_partial_file_creation() {
    let root = TestRoot::new("content-length");
    let fetcher = MediaFetcher::new(
        FakeDns::public(),
        FakeHttp::new([response(200, None, Some(33), vec![])]),
        download_store(&root).await,
        limits(32),
    )
    .expect("fetcher");

    let error = fetcher
        .fetch(MediaRequest::DirectUrl {
            url: "https://media.example/too-large".to_owned(),
        })
        .await
        .expect_err("length bound");

    assert!(matches!(error, FetchError::ContentLengthTooLarge));
    for directory in [root.0.join("staging"), root.0.join("objects")] {
        assert_eq!(
            std::fs::read_dir(directory)
                .expect("managed directory")
                .count(),
            0,
            "no partial may be allocated"
        );
    }
}

#[tokio::test]
async fn partial_is_removed_when_stream_fails_after_writing() {
    let root = TestRoot::new("cleanup");
    let fetcher = MediaFetcher::new(
        FakeDns::public(),
        FakeHttp::new([response(
            200,
            None,
            None,
            vec![Ok(b"partial"), Err(BodyError)],
        )]),
        download_store(&root).await,
        limits(32),
    )
    .expect("fetcher");

    let error = fetcher
        .fetch(MediaRequest::DirectUrl {
            url: "https://media.example/fails".to_owned(),
        })
        .await
        .expect_err("body failure");

    assert!(matches!(error, FetchError::Body));
    for directory in [root.0.join("staging"), root.0.join("objects")] {
        assert_eq!(
            std::fs::read_dir(directory)
                .expect("managed directory")
                .count(),
            0
        );
    }
}

#[tokio::test]
async fn measured_bytes_enforce_cap_without_content_length() {
    let root = TestRoot::new("byte-cap");
    let fetcher = MediaFetcher::new(
        FakeDns::public(),
        FakeHttp::new([response(200, None, None, vec![Ok(b"1234"), Ok(b"5")])]),
        download_store(&root).await,
        limits(4),
    )
    .expect("fetcher");

    let error = fetcher
        .fetch(MediaRequest::DirectUrl {
            url: "https://media.example/growing".to_owned(),
        })
        .await
        .expect_err("measured cap");

    assert!(matches!(error, FetchError::DownloadTooLarge));
    assert_eq!(
        std::fs::read_dir(root.0.join("objects"))
            .expect("objects")
            .count(),
        0
    );
}

#[tokio::test(start_paused = true)]
async fn stalled_body_hits_download_deadline_and_removes_partial() {
    let root = TestRoot::new("timeout");
    let response = HttpResponse::new(
        200,
        None,
        None,
        None,
        Box::pin(stream::pending::<Result<Bytes, BodyError>>()),
    );
    let fetcher = MediaFetcher::new(
        FakeDns::public(),
        FakeHttp::new([response]),
        download_store(&root).await,
        FetchLimits {
            download_timeout: Duration::from_secs(30),
            ..limits(32)
        },
    )
    .expect("fetcher");

    let error = fetcher
        .fetch(MediaRequest::DirectUrl {
            url: "https://media.example/stalls".to_owned(),
        })
        .await
        .expect_err("download timeout");

    assert!(matches!(error, FetchError::DownloadTimeout));
    assert_eq!(
        std::fs::read_dir(root.0.join("staging"))
            .expect("staging")
            .count(),
        0
    );
}

#[tokio::test]
async fn declared_attachment_size_rejects_before_dns_or_http() {
    let root = TestRoot::new("declared-size");
    let dns = FakeDns::public();
    let fetcher = MediaFetcher::new(
        dns.clone(),
        FakeHttp::new([]),
        download_store(&root).await,
        limits(4),
    )
    .expect("fetcher");

    let error = fetcher
        .fetch(MediaRequest::DiscordAttachment(DiscordAttachment {
            url: "https://cdn.example/a".to_owned(),
            filename: "a".to_owned(),
            content_type: None,
            declared_size_bytes: Some(5),
        }))
        .await
        .expect_err("declared cap");

    assert!(matches!(error, FetchError::DeclaredSizeTooLarge));
    assert_eq!(dns.call_count(), 0);
}

#[tokio::test]
async fn redirect_loop_is_detected() {
    let root = TestRoot::new("loop");
    let fetcher = MediaFetcher::new(
        FakeDns::public(),
        FakeHttp::new([response(302, Some("/start"), None, vec![])]),
        download_store(&root).await,
        FetchLimits {
            max_redirects: 3,
            ..limits(32)
        },
    )
    .expect("fetcher");

    let error = fetcher
        .fetch(MediaRequest::DirectUrl {
            url: "https://media.example/start".to_owned(),
        })
        .await
        .expect_err("redirect loop");

    assert!(matches!(error, FetchError::RedirectLoop));
}

#[test]
fn default_time_bounds_are_nonzero() {
    let limits = FetchLimits::default();
    assert!(limits.redirect_timeout > Duration::ZERO);
    assert!(limits.download_timeout > Duration::ZERO);
}
