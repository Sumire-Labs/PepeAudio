use std::{collections::VecDeque, ffi::OsString, path::PathBuf, sync::Mutex, time::Duration};

use async_trait::async_trait;

use crate::{CommandSpec, OutputLimits, ProcessError, ProcessOutput, ProcessRunner};

use super::{SiteError, SiteProvider, SiteSearch, YtDlpClient, YtDlpConfig};

struct FakeRunner {
    outputs: Mutex<VecDeque<Result<ProcessOutput, ProcessError>>>,
    commands: Mutex<Vec<CommandSpec>>,
}

impl FakeRunner {
    fn json(values: impl IntoIterator<Item = &'static str>) -> Self {
        Self::results(values.into_iter().map(|value| {
            Ok(ProcessOutput {
                status_code: Some(0),
                stdout: value.as_bytes().to_vec(),
                stderr: Vec::new(),
            })
        }))
    }

    fn results(values: impl IntoIterator<Item = Result<ProcessOutput, ProcessError>>) -> Self {
        Self {
            outputs: Mutex::new(values.into_iter().collect()),
            commands: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl ProcessRunner for FakeRunner {
    async fn run(
        &self,
        specification: &CommandSpec,
        _limits: OutputLimits,
    ) -> Result<ProcessOutput, ProcessError> {
        self.commands
            .lock()
            .expect("commands")
            .push(specification.clone());
        self.outputs
            .lock()
            .expect("outputs")
            .pop_front()
            .expect("fixture output")
    }
}

fn config() -> YtDlpConfig {
    YtDlpConfig {
        executable: PathBuf::from("yt-dlp"),
        deno_executable: PathBuf::from("deno"),
        deno_directory: PathBuf::from("/tmp/pepeaudio-deno"),
        maximum_track_duration: Duration::from_hours(6),
        maximum_playlist_items: 25,
    }
}

#[tokio::test]
async fn startup_verifies_supported_tool_versions_with_bounded_commands() {
    let runner = std::sync::Arc::new(FakeRunner::json([
        "2026.06.09\n",
        "deno 2.8.1 (stable, release, x86_64-unknown-linux-gnu)\n",
    ]));
    let client = YtDlpClient::new(config(), runner.clone()).expect("client");

    client.verify_tools().await.expect("supported tools");

    let commands = runner.commands.lock().expect("commands");
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].program(), std::path::Path::new("yt-dlp"));
    assert_eq!(commands[1].program(), std::path::Path::new("deno"));
    assert!(commands.iter().all(
        |command| command.deno_directory() == Some(std::path::Path::new("/tmp/pepeaudio-deno"))
    ));
}

#[tokio::test]
async fn unavailable_playlist_item_is_skippable_but_operational_exit_is_not() {
    let runner = std::sync::Arc::new(FakeRunner::results([
        Ok(ProcessOutput {
            status_code: Some(0),
            stdout: br#"{"_type":"video","title":"Track"}"#.to_vec(),
            stderr: Vec::new(),
        }),
        Err(ProcessError::MediaUnavailable),
    ]));
    let client = YtDlpClient::new(config(), runner).expect("client");
    let collection = client
        .discover_url("https://youtu.be/abcdefghijk", 1)
        .await
        .expect("discover");
    assert!(matches!(
        client.resolve(&collection.entries[0]).await,
        Err(SiteError::UnsupportedStream)
    ));

    let runner = std::sync::Arc::new(FakeRunner::results([Err(ProcessError::Exit {
        code: Some(1),
    })]));
    let client = YtDlpClient::new(config(), runner).expect("client");
    assert!(matches!(
        client.discover_url("https://youtu.be/abcdefghijk", 1).await,
        Err(SiteError::Process(ProcessError::Exit { code: Some(1) }))
    ));
}

#[tokio::test]
async fn startup_rejects_old_or_unparseable_tool_versions() {
    for outputs in [
        ["2026.06.08\n", "deno 2.8.1\n"],
        ["2026.06.09\n", "deno 1.46.0\n"],
        ["not-a-version\n", "deno 2.8.1\n"],
    ] {
        let runner = std::sync::Arc::new(FakeRunner::json(outputs));
        let client = YtDlpClient::new(config(), runner).expect("client");
        assert!(matches!(
            client.verify_tools().await,
            Err(SiteError::UnsupportedToolVersion)
        ));
    }
}

#[test]
fn provider_policy_accepts_only_expected_https_page_hosts() {
    assert_eq!(
        SiteProvider::classify("https://www.youtube.com/watch?v=abcdefghijk").expect("YouTube"),
        Some(SiteProvider::YouTube)
    );
    assert_eq!(
        SiteProvider::classify("https://on.soundcloud.com/example").expect("SoundCloud"),
        Some(SiteProvider::SoundCloud)
    );
    assert_eq!(
        SiteProvider::classify("https://youtube.com.evil.test/watch?v=abcdefghijk")
            .expect("lookalike remains a direct URL"),
        None
    );
    for rejected in [
        "http://youtube.com/watch?v=abcdefghijk",
        "https://user@youtube.com/watch?v=abcdefghijk",
        "https://youtube.com:444/watch?v=abcdefghijk",
        "https://youtube.com/watch?v=abcdefghijk#fragment",
        "https://youtube.com/watch?v=abcdefghijk\r\n--cookies=x",
        "https://youtube.com/watch?v=abc\0def",
    ] {
        assert!(SiteProvider::classify(rejected).is_err(), "{rejected}");
    }
    assert_eq!(
        SiteProvider::classify("https://example.test/audio.ogg").expect("direct URL"),
        None
    );
}

#[tokio::test]
async fn commands_disable_ambient_configuration_and_select_safe_direct_audio() {
    let runner = std::sync::Arc::new(FakeRunner::json([
        r#"{"_type":"video","title":"Track"}"#,
        r#"{"title":"Track","duration":120,"protocol":"https","vcodec":"none","acodec":"opus","url":"https://rr1.googlevideo.com/videoplayback?token=secret","http_headers":{"User-Agent":"safe-agent","Sec-Fetch-Mode":"navigate"}}"#,
    ]));
    let client = YtDlpClient::new(config(), runner.clone()).expect("client");
    let collection = client
        .discover_url("https://youtu.be/abcdefghijk", 25)
        .await
        .expect("discover");
    let resolved = client
        .resolve(&collection.entries[0])
        .await
        .expect("resolve");

    assert_eq!(resolved.title, "Track");
    let commands = runner.commands.lock().expect("commands");
    let arguments = commands
        .iter()
        .flat_map(CommandSpec::arguments)
        .map(|value| value.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    for required in [
        "--ignore-config",
        "--no-config-locations",
        "--no-plugin-dirs",
        "--no-remote-components",
        "--no-cookies",
        "--no-exec",
    ] {
        assert!(arguments.iter().any(|argument| argument == required));
    }
    assert!(!arguments.iter().any(|argument| argument == "--no-netrc"));
    assert!(
        arguments
            .iter()
            .any(|argument| { argument == "bestaudio[protocol=https][vcodec=none][acodec!=none]" })
    );
    assert!(commands.iter().all(
        |command| command.deno_directory() == Some(std::path::Path::new("/tmp/pepeaudio-deno"))
    ));
    assert_eq!(
        commands[0].arguments().last(),
        Some(&OsString::from("https://youtu.be/abcdefghijk"))
    );
    assert!(!commands[0].should_classify_unavailable_media());
    assert!(commands[1].should_classify_unavailable_media());
}

#[tokio::test]
async fn playlist_is_bounded_and_reports_truncation() {
    let runner = std::sync::Arc::new(FakeRunner::json([r#"{
        "_type":"playlist","title":"Long list","playlist_count":40,"entries":[
          {"id":"abcdefghijk","title":"one"},
          {"id":"lmnopqrstuv","title":"two"},
          {"id":"wxyzABCDEF0","title":"three"}
        ]}"#]));
    let client = YtDlpClient::new(config(), runner).expect("client");
    let collection = client
        .discover_url("https://www.youtube.com/playlist?list=PLfixture", 2)
        .await
        .expect("bounded playlist");

    assert_eq!(collection.entries.len(), 2);
    assert_eq!(collection.source_item_count, Some(40));
    assert_eq!(collection.skipped_items, 0);
    assert!(collection.truncated);
}

#[tokio::test]
async fn unknown_playlist_total_is_not_invented_and_invalid_entries_are_counted() {
    let runner = std::sync::Arc::new(FakeRunner::json([r#"{
        "_type":"playlist","title":"Partial list","entries":[
          {"id":"abcdefghijk","title":"one"},
          null,
          {"id":"lmnopqrstuv","title":"three"}
        ]}"#]));
    let client = YtDlpClient::new(config(), runner).expect("client");
    let collection = client
        .discover_url("https://www.youtube.com/playlist?list=PLfixture", 2)
        .await
        .expect("bounded playlist");

    assert_eq!(collection.entries.len(), 1);
    assert_eq!(collection.source_item_count, None);
    assert_eq!(collection.skipped_items, 1);
    assert!(collection.truncated);
}

#[tokio::test]
async fn playlist_with_only_private_or_invalid_entries_has_an_explicit_item_failure() {
    let runner = std::sync::Arc::new(FakeRunner::json([r#"{
        "_type":"playlist","title":"Unavailable list","entries":[null,{}]
    }"#]));
    let client = YtDlpClient::new(config(), runner).expect("client");

    assert!(matches!(
        client
            .discover_url("https://www.youtube.com/playlist?list=PLfixture", 2)
            .await,
        Err(SiteError::UnsupportedStream)
    ));
}

fn catalog_search() -> SiteSearch {
    SiteSearch::new(
        "Example Song Primary Artist",
        "Example Song",
        vec!["Primary Artist".into()],
        Some(180_000),
        Some("JPABC1234567".into()),
    )
    .expect("search")
}

#[tokio::test]
async fn empty_youtube_search_falls_back_to_a_strong_soundcloud_match() {
    let runner = std::sync::Arc::new(FakeRunner::json([
        r#"{"_type":"playlist","entries":[]}"#,
        r#"{"_type":"playlist","entries":[{"webpage_url":"https://soundcloud.com/primary-artist/example-song","title":"Example Song","uploader":"Primary Artist","duration":180}]}"#,
        r#"{"title":"Example Song","duration":180,"protocol":"http","vcodec":"none","acodec":"mp3","url":"https://cf-media.sndcdn.com/media/example.128.mp3","http_headers":{}}"#,
    ]));
    let client = YtDlpClient::new(config(), runner.clone()).expect("client");

    let resolved = client
        .resolve_search(&catalog_search())
        .await
        .expect("SoundCloud fallback");

    assert_eq!(resolved.title, "Example Song");
    assert_eq!(runner.commands.lock().expect("commands").len(), 3);
}

#[tokio::test]
async fn full_metadata_duration_is_rechecked_after_flat_search_ranking() {
    let runner = std::sync::Arc::new(FakeRunner::json([
        r#"{"_type":"playlist","entries":[{"id":"abcdefghijk","title":"Example Song","uploader":"Primary Artist","duration":180}]}"#,
        r#"{"title":"Example Song","duration":400,"protocol":"https","vcodec":"none","acodec":"opus","url":"https://rr1.googlevideo.com/videoplayback","http_headers":{}}"#,
        r#"{"_type":"playlist","entries":[]}"#,
    ]));
    let client = YtDlpClient::new(config(), runner).expect("client");

    assert!(matches!(
        client.resolve_search(&catalog_search()).await,
        Err(SiteError::NoSearchMatch)
    ));
}

#[tokio::test]
async fn an_unsupported_top_candidate_advances_to_the_next_ranked_match() {
    let runner = std::sync::Arc::new(FakeRunner::json([
        r#"{"_type":"playlist","entries":[
          {"id":"abcdefghijk","title":"Example Song","uploader":"Primary Artist","duration":180},
          {"id":"lmnopqrstuv","title":"Example Song Official Audio","uploader":"Primary Artist","duration":188}
        ]}"#,
        r#"{"title":"Example Song","duration":180,"protocol":"m3u8_native","vcodec":"none","acodec":"aac","url":"https://rr1.googlevideo.com/manifest.m3u8","http_headers":{}}"#,
        r#"{"title":"Example Song","duration":188,"protocol":"https","vcodec":"none","acodec":"opus","url":"https://rr1.googlevideo.com/videoplayback","http_headers":{}}"#,
    ]));
    let client = YtDlpClient::new(config(), runner).expect("client");

    let resolved = client
        .resolve_search(&catalog_search())
        .await
        .expect("second safe candidate");

    assert_eq!(resolved.duration_ms, 188_000);
}

#[tokio::test]
async fn operational_and_security_failures_are_not_masked_as_search_misses() {
    let runner = std::sync::Arc::new(FakeRunner::results([Err(ProcessError::Timeout)]));
    let client = YtDlpClient::new(config(), runner).expect("client");
    assert!(matches!(
        client.resolve_search(&catalog_search()).await,
        Err(SiteError::Process(ProcessError::Timeout))
    ));

    let runner = std::sync::Arc::new(FakeRunner::json([
        r#"{"_type":"playlist","entries":[{"id":"abcdefghijk","title":"Example Song","uploader":"Primary Artist","duration":180}]}"#,
        r#"{"title":"Example Song","duration":180,"protocol":"https","vcodec":"none","acodec":"opus","url":"https://rr1.googlevideo.com/videoplayback","http_headers":{"Authorization":"secret"}}"#,
    ]));
    let client = YtDlpClient::new(config(), runner).expect("client");
    assert!(matches!(
        client.resolve_search(&catalog_search()).await,
        Err(SiteError::UnsafeHeader)
    ));
}

#[tokio::test]
async fn soundcloud_http_protocol_label_still_requires_an_https_cdn_url() {
    let runner = std::sync::Arc::new(FakeRunner::json([
        r#"{"_type":"video","title":"Track"}"#,
        r#"{"title":"Track","duration":120,"protocol":"http","vcodec":"none","acodec":"mp3","url":"https://cf-media.sndcdn.com/media/example.128.mp3","http_headers":{}}"#,
    ]));
    let client = YtDlpClient::new(config(), runner).expect("client");
    let collection = client
        .discover_url("https://soundcloud.com/artist/track", 1)
        .await
        .expect("discover");
    assert!(client.resolve(&collection.entries[0]).await.is_ok());
}

#[tokio::test]
async fn authentication_headers_and_manifest_streams_fail_closed() {
    for json in [
        r#"{"title":"Track","duration":120,"protocol":"https","vcodec":"none","acodec":"opus","url":"https://rr1.googlevideo.com/videoplayback","http_headers":{"Cookie":"secret=1"}}"#,
        r#"{"title":"Track","duration":120,"protocol":"m3u8_native","vcodec":"none","acodec":"aac","url":"https://rr1.googlevideo.com/manifest.m3u8","http_headers":{}}"#,
    ] {
        let runner = std::sync::Arc::new(FakeRunner::json([
            r#"{"_type":"video","title":"Track"}"#,
            json,
        ]));
        let client = YtDlpClient::new(config(), runner).expect("client");
        let collection = client
            .discover_url("https://youtu.be/abcdefghijk", 1)
            .await
            .expect("discover");
        assert!(matches!(
            client.resolve(&collection.entries[0]).await,
            Err(SiteError::UnsafeHeader | SiteError::UnsupportedStream)
        ));
    }
}
