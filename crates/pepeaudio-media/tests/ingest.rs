mod support;

use std::{
    path::Path,
    sync::Arc,
    time::{Duration, SystemTime},
};

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream;
use pepeaudio_media::{
    DownloadStore, FetchLimits, HttpResponse, InspectedMedia, JanitorClock, JanitorPolicy,
    ManagedDownloadJanitor, ManagedMediaLeaseRegistry, MediaFetcher, MediaIngestor, MediaProbe,
    MediaRequest, ProbeMetadata, ProbeStream, ProcessError,
};
use support::{FakeDns, FakeHttp, TestRoot, download_store};
use tokio::sync::Notify;

struct FakeProbe(Result<ProbeMetadata, ProcessError>);

#[derive(Clone)]
struct BlockingProbe {
    started: Arc<Notify>,
}

#[derive(Clone, Copy)]
struct FixedClock(SystemTime);

impl JanitorClock for FixedClock {
    fn now(&self) -> SystemTime {
        self.0
    }
}

#[async_trait]
impl MediaProbe for FakeProbe {
    async fn probe(&self, _path: &Path) -> Result<ProbeMetadata, ProcessError> {
        match &self.0 {
            Ok(metadata) => Ok(metadata.clone()),
            Err(_) => Err(ProcessError::InvalidProbe),
        }
    }
}

#[async_trait]
impl MediaProbe for BlockingProbe {
    async fn probe(&self, _path: &Path) -> Result<ProbeMetadata, ProcessError> {
        self.started.notify_one();
        std::future::pending().await
    }
}

async fn fetcher(root: &TestRoot) -> MediaFetcher<FakeDns, FakeHttp> {
    MediaFetcher::new(
        FakeDns::public(),
        FakeHttp::new([HttpResponse::new(
            200,
            None,
            Some(4),
            None,
            Box::pin(stream::once(async { Ok(Bytes::from_static(b"data")) })),
        )]),
        download_store(root).await,
        FetchLimits::default(),
    )
    .expect("fetcher")
}

#[tokio::test]
async fn ingestion_returns_download_and_validated_metadata() {
    let root = TestRoot::new("ingest-success");
    let metadata = ProbeMetadata {
        format_name: Some("fixture".to_owned()),
        duration_seconds: Some(1.0),
        audio_streams: vec![ProbeStream {
            index: 0,
            codec_name: Some("pcm".to_owned()),
            sample_rate_hz: Some(48_000),
            channels: Some(2),
            channel_layout: Some("stereo".to_owned()),
        }],
    };
    let ingestor = MediaIngestor::new(fetcher(&root).await, FakeProbe(Ok(metadata.clone())));

    let InspectedMedia {
        download,
        metadata: actual,
    } = ingestor
        .ingest(MediaRequest::DirectUrl {
            url: "https://media.example/object".to_owned(),
        })
        .await
        .expect("ingested");

    assert_eq!(actual, metadata);
    assert!(download.path.exists());
}

#[tokio::test]
async fn rejected_probe_removes_completed_object() {
    let root = TestRoot::new("ingest-reject");
    let ingestor = MediaIngestor::new(
        fetcher(&root).await,
        FakeProbe(Err(ProcessError::InvalidProbe)),
    );

    let error = ingestor
        .ingest(MediaRequest::DirectUrl {
            url: "https://media.example/object".to_owned(),
        })
        .await
        .expect_err("invalid probe");

    assert!(matches!(error, pepeaudio_media::IngestError::Probe(_)));
    assert_eq!(
        std::fs::read_dir(root.0.join("objects"))
            .expect("objects")
            .count(),
        0
    );
}

#[tokio::test]
async fn cancelled_probe_keeps_committed_bytes_metered_until_janitor_reclaims_them() {
    let root = TestRoot::new("ingest-cancelled-probe");
    let registry = ManagedMediaLeaseRegistry::new_with_capacity(&root.0, 1024, 32)
        .await
        .expect("capacity registry");
    let store = DownloadStore::new(registry.clone()).expect("download store");
    let fetcher = MediaFetcher::new(
        FakeDns::public(),
        FakeHttp::new([HttpResponse::new(
            200,
            None,
            Some(4),
            None,
            Box::pin(stream::once(async { Ok(Bytes::from_static(b"data")) })),
        )]),
        store,
        FetchLimits {
            max_download_bytes: 16,
            ..FetchLimits::default()
        },
    )
    .expect("fetcher");
    let probe_started = Arc::new(Notify::new());
    let ingestor = MediaIngestor::new(
        fetcher,
        BlockingProbe {
            started: Arc::clone(&probe_started),
        },
    );
    let task = tokio::spawn(async move {
        ingestor
            .ingest(MediaRequest::DirectUrl {
                url: "https://media.example/object".to_owned(),
            })
            .await
    });

    probe_started.notified().await;
    task.abort();
    assert!(task.await.expect_err("cancelled task").is_cancelled());

    let object = std::fs::read_dir(root.0.join("objects"))
        .expect("objects")
        .next()
        .expect("committed object")
        .expect("object entry")
        .path();
    let usage = registry.capacity_usage().expect("capacity accounting");
    assert_eq!(usage.used_bytes, 4);
    assert_eq!(usage.reserved_bytes, 0);

    let modified = std::fs::metadata(&object)
        .expect("object metadata")
        .modified()
        .expect("object modification time");
    let janitor = ManagedDownloadJanitor::with_clock_and_registry(
        &root.0,
        JanitorPolicy {
            staging_ttl: Duration::from_secs(1),
            object_ttl: Duration::from_secs(1),
            minimum_object_retention: Duration::from_secs(1),
            max_total_bytes: 1024,
            max_entries_per_scan: 32,
            dry_run: false,
        },
        FixedClock(modified + Duration::from_secs(2)),
        registry.clone(),
    )
    .await
    .expect("janitor");

    let report = janitor.run().await.expect("janitor run");
    assert_eq!(report.removals.len(), 1);
    assert!(!object.exists());
    let usage = registry.capacity_usage().expect("capacity accounting");
    assert_eq!(usage.used_bytes, 0);
    assert_eq!(usage.reserved_bytes, 0);
}
