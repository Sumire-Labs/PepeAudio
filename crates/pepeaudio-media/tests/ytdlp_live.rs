use std::{env, path::PathBuf, sync::Arc, time::Duration};

use pepeaudio_media::{
    DownloadStore, FetchLimits, Ffprobe, ManagedMediaLeaseRegistry, MediaFetcher, MediaIngestor,
    OutputLimits, ProcessPool, RealProcessRunner, ReqwestTransport, TokioDnsResolver, YtDlpClient,
    YtDlpConfig,
};
use uuid::Uuid;

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        Self(env::temp_dir().join(format!("pepeaudio-ytdlp-live-{}", Uuid::new_v4().simple())))
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
#[ignore = "uses live YouTube, yt-dlp, Deno, ffprobe, and public network access"]
async fn youtube_audio_uses_the_managed_fetch_and_probe_pipeline() {
    let page = env::var("PEPEAUDIO_YOUTUBE_SMOKE_URL")
        .expect("set an explicitly authorized YouTube smoke URL");
    exercise_page(&page, "googlevideo.com").await;
}

#[tokio::test]
#[ignore = "uses live SoundCloud, yt-dlp, ffprobe, and public network access"]
async fn soundcloud_audio_uses_the_managed_fetch_and_probe_pipeline() {
    let page = env::var("PEPEAUDIO_SOUNDCLOUD_SMOKE_URL")
        .expect("set an explicitly authorized SoundCloud smoke URL");
    exercise_page(&page, "sndcdn.com").await;
}

async fn exercise_page(page: &str, private_media_host: &str) {
    let root = TestRoot::new();
    let runner = RealProcessRunner::new(ProcessPool::new(2).expect("process pool"));
    let client = YtDlpClient::new(
        YtDlpConfig {
            executable: env_path("PEPEAUDIO_YTDLP_PATH", "yt-dlp"),
            deno_executable: env_path("PEPEAUDIO_DENO_PATH", "deno"),
            deno_directory: env_path("PEPEAUDIO_DENO_DIR", root.0.join("deno").as_os_str()),
            maximum_track_duration: Duration::from_mins(4),
            maximum_playlist_items: 1,
        },
        Arc::new(runner.clone()),
    )
    .expect("yt-dlp client");
    client.verify_tools().await.expect("pinned tools available");

    let collection = client
        .discover_url(page, 1)
        .await
        .expect("discover provider page");
    let resolved = client
        .resolve(collection.entries.first().expect("one video"))
        .await
        .expect("resolve direct audio");
    let debug = format!("{resolved:?}");
    assert!(!debug.contains(private_media_host));
    assert!(!debug.contains("signature="));

    let capacity = 32 * 1024 * 1024;
    let registry = ManagedMediaLeaseRegistry::new_with_capacity(&root.0, capacity, 128)
        .await
        .expect("managed capacity");
    let store = DownloadStore::new(registry.clone()).expect("download store");
    let fetcher = MediaFetcher::new(
        TokioDnsResolver,
        ReqwestTransport,
        store,
        FetchLimits {
            max_download_bytes: capacity,
            ..FetchLimits::default()
        },
    )
    .expect("media fetcher");
    let probe = Ffprobe::new(
        env_path("PEPEAUDIO_FFPROBE_PATH", "ffprobe"),
        runner,
        OutputLimits {
            timeout: Duration::from_secs(30),
            max_stdout_bytes: 1024 * 1024,
            max_stderr_bytes: 64 * 1024,
        },
    );
    let ingestor = MediaIngestor::new(fetcher, probe);
    let inspected = ingestor
        .ingest(resolved.request)
        .await
        .expect("bounded fetch and probe");
    assert!(inspected.download.size_bytes > 0);
    assert!(!inspected.metadata.audio_streams.is_empty());
    let object = inspected.download.path.clone();
    ingestor.discard(&object).await.expect("managed cleanup");
    assert!(!object.exists());
    assert_eq!(
        registry
            .capacity_usage()
            .expect("capacity usage")
            .reserved_bytes,
        0
    );
}

fn env_path(name: &str, fallback: impl AsRef<std::ffi::OsStr>) -> PathBuf {
    env::var_os(name).map_or_else(|| PathBuf::from(fallback.as_ref()), PathBuf::from)
}
