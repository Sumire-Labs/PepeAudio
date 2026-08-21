mod support;

use std::{path::PathBuf, sync::Arc, time::Duration};

use bytes::Bytes;
use futures_util::stream;
use pepeaudio_media::{
    BodyError, FetchLimits, HttpResponse, MediaFetcher, MediaRequest, MediaSourceKind, YtDlpClient,
    YtDlpConfig,
};
use support::{FakeDns, FakeHttp, FakeProcess, TestRoot, download_store};

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
async fn youtube_open_range_is_preserved_across_approved_redirects() {
    let root = TestRoot::new("youtube-range-redirect");
    let http = FakeHttp::new([
        response(
            302,
            Some("https://rr2.googlevideo.com/videoplayback?token=next"),
            Some(0),
            vec![],
        ),
        response(200, None, Some(6), vec![Ok(b"media!")]),
    ]);
    let calls = http.clone();
    let fetcher = MediaFetcher::new(
        FakeDns::public(),
        http,
        download_store(&root).await,
        FetchLimits {
            max_download_bytes: 32,
            ..FetchLimits::default()
        },
    )
    .expect("fetcher");
    let request = resolved_site_request(
        "https://youtu.be/abcdefghijk",
        r#"{"title":"Track","id":"abcdefghijk","webpage_url":"https://www.youtube.com/watch?v=abcdefghijk","duration":212,"protocol":"https","vcodec":"none","acodec":"opus","url":"https://rr1.googlevideo.com/videoplayback?token=first","http_headers":{"Range":"bytes=500-999"}}"#,
    )
    .await;

    let downloaded = fetcher.fetch(request).await.expect("YouTube download");

    assert_eq!(downloaded.source_kind, MediaSourceKind::ResolvedSite);
    assert_eq!(
        calls.calls(),
        [
            "https://rr1.googlevideo.com/videoplayback?token=first",
            "https://rr2.googlevideo.com/videoplayback?token=next"
        ]
    );
    assert_eq!(calls.open_range_calls(), [true, true]);
}

#[tokio::test]
async fn soundcloud_resolved_media_does_not_use_open_range() {
    let root = TestRoot::new("soundcloud-no-range");
    let http = FakeHttp::new([response(200, None, Some(4), vec![Ok(b"data")])]);
    let calls = http.clone();
    let fetcher = MediaFetcher::new(
        FakeDns::public(),
        http,
        download_store(&root).await,
        FetchLimits {
            max_download_bytes: 32,
            ..FetchLimits::default()
        },
    )
    .expect("fetcher");
    let request = resolved_site_request(
        "https://soundcloud.com/artist/track",
        r#"{"title":"Track","webpage_url":"https://soundcloud.com/artist/track","duration":120,"protocol":"http","vcodec":"none","acodec":"mp3","url":"https://cf-media.sndcdn.com/media/example.128.mp3","http_headers":{}}"#,
    )
    .await;

    fetcher.fetch(request).await.expect("SoundCloud download");

    assert_eq!(calls.open_range_calls(), [false]);
}

async fn resolved_site_request(page: &str, resolved_json: &'static str) -> MediaRequest {
    let runner = Arc::new(FakeProcess::json([
        r#"{"_type":"video","title":"Track"}"#,
        resolved_json,
    ]));
    let client = YtDlpClient::new(
        YtDlpConfig {
            executable: PathBuf::from("yt-dlp"),
            deno_executable: PathBuf::from("deno"),
            deno_directory: PathBuf::from("test-deno"),
            maximum_track_duration: Duration::from_mins(5),
            maximum_playlist_items: 1,
        },
        runner,
    )
    .expect("site client");
    let collection = client.discover_url(page, 1).await.expect("site discovery");
    client
        .resolve(collection.entries.first().expect("one site entry"))
        .await
        .expect("site resolution")
        .request
}
