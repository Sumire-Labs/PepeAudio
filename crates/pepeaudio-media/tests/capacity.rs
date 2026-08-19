mod support;

use std::{path::Path, sync::Arc, time::Duration, time::SystemTime};

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream;
use pepeaudio_media::{
    DiscordAttachment, FetchError, FetchLimits, HttpResponse, JanitorClock, JanitorPolicy,
    ManagedDownloadJanitor, ManagedMediaLeaseRegistry, MediaFetcher, MediaIngestor, MediaProbe,
    MediaRequest, ProbeMetadata, ProcessError, StoreError,
};
use support::{FakeDns, FakeHttp, TestRoot};

const OBJECT_A: &str = "0000000000000000000000000000000a";
const OBJECT_B: &str = "0000000000000000000000000000000b";
const PARTIAL: &str = "0000000000000000000000000000000c.part";

struct AcceptProbe;

#[async_trait]
impl MediaProbe for AcceptProbe {
    async fn probe(&self, _path: &Path) -> Result<ProbeMetadata, ProcessError> {
        Ok(ProbeMetadata {
            format_name: Some("ogg".into()),
            duration_seconds: Some(1.0),
            audio_streams: Vec::new(),
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct FixedClock(SystemTime);

impl JanitorClock for FixedClock {
    fn now(&self) -> SystemTime {
        self.0
    }
}

fn limits(maximum: u64) -> FetchLimits {
    FetchLimits {
        max_download_bytes: maximum,
        ..FetchLimits::default()
    }
}

fn response(bytes: &'static [u8]) -> HttpResponse {
    HttpResponse::new(
        200,
        None,
        Some(bytes.len() as u64),
        None,
        Box::pin(stream::once(async move { Ok(Bytes::from_static(bytes)) })),
    )
}

async fn managed_file(root: &Path, directory: &str, name: &str, bytes: &[u8]) {
    let directory = root.join(directory);
    tokio::fs::create_dir_all(&directory)
        .await
        .expect("managed directory");
    tokio::fs::write(directory.join(name), bytes)
        .await
        .expect("managed file");
}

#[tokio::test]
async fn startup_counts_objects_and_staging_exactly() {
    let root = TestRoot::new("capacity-startup");
    managed_file(&root.0, "objects", OBJECT_A, b"abcd").await;
    managed_file(&root.0, "staging", PARTIAL, b"xyz").await;

    let registry = ManagedMediaLeaseRegistry::new_with_capacity(&root.0, 32, 4)
        .await
        .expect("safe startup scan");

    assert_eq!(registry.maximum_bytes(), Some(32));
    assert_eq!(registry.charged_bytes(), Some(7));
}

#[tokio::test]
async fn startup_fails_closed_for_unknown_or_unbounded_entries() {
    let unknown = TestRoot::new("capacity-unknown");
    managed_file(&unknown.0, "objects", "operator-note", b"unknown").await;
    assert!(
        ManagedMediaLeaseRegistry::new_with_capacity(&unknown.0, 32, 4)
            .await
            .is_err()
    );

    let bounded = TestRoot::new("capacity-bounded");
    managed_file(&bounded.0, "objects", OBJECT_A, b"a").await;
    managed_file(&bounded.0, "objects", OBJECT_B, b"b").await;
    assert!(
        ManagedMediaLeaseRegistry::new_with_capacity(&bounded.0, 32, 1)
            .await
            .is_err()
    );

    let over_budget = TestRoot::new("capacity-over-budget");
    managed_file(&over_budget.0, "objects", OBJECT_A, b"12345").await;
    assert!(
        ManagedMediaLeaseRegistry::new_with_capacity(&over_budget.0, 4, 4)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn full_root_rejects_before_dns_or_http() {
    let root = TestRoot::new("capacity-pre-network");
    managed_file(&root.0, "objects", OBJECT_A, b"12345678").await;
    let registry = ManagedMediaLeaseRegistry::new_with_capacity(&root.0, 8, 4)
        .await
        .expect("registry");
    let dns = FakeDns::public();
    let http = FakeHttp::new([]);
    let calls = http.clone();
    let store = pepeaudio_media::DownloadStore::new(registry).expect("store");
    let fetcher = MediaFetcher::new(dns.clone(), http, store, limits(4)).expect("fetcher");

    let error = fetcher
        .fetch(MediaRequest::DirectUrl {
            url: "https://media.example/full".into(),
        })
        .await
        .expect_err("capacity rejection");

    assert!(matches!(error, FetchError::AdmissionCapacityExceeded));
    assert_eq!(dns.call_count(), 0);
    assert!(calls.calls().is_empty());
}

#[tokio::test]
async fn dishonest_declared_size_reports_post_network_capacity_separately() {
    let root = TestRoot::new("capacity-declared-growth");
    managed_file(&root.0, "objects", OBJECT_A, b"abc").await;
    let registry = ManagedMediaLeaseRegistry::new_with_capacity(&root.0, 6, 4)
        .await
        .expect("registry");
    let http = FakeHttp::new([response(b"wxyz")]);
    let calls = http.clone();
    let store = pepeaudio_media::DownloadStore::new(registry).expect("store");
    let fetcher = MediaFetcher::new(FakeDns::public(), http, store, limits(4)).expect("fetcher");

    let error = fetcher
        .fetch(MediaRequest::DiscordAttachment(DiscordAttachment {
            url: "https://media.example/grows".into(),
            filename: "grows.ogg".into(),
            content_type: None,
            declared_size_bytes: Some(2),
        }))
        .await
        .expect_err("header growth no longer fits");

    assert!(matches!(
        error,
        FetchError::Store(StoreError::CapacityExceeded)
    ));
    assert_eq!(calls.calls().len(), 1);
}

#[tokio::test]
async fn concurrent_reservation_is_exclusive_and_cancel_releases_it() {
    let root = TestRoot::new("capacity-concurrent");
    let registry = ManagedMediaLeaseRegistry::new_with_capacity(&root.0, 4, 4)
        .await
        .expect("registry");
    let pending = HttpResponse::new(200, None, None, None, Box::pin(stream::pending()));
    let http = FakeHttp::new([pending, response(b"abc")]);
    let calls = http.clone();
    let store = pepeaudio_media::DownloadStore::new(registry.clone()).expect("store");
    let fetcher =
        Arc::new(MediaFetcher::new(FakeDns::public(), http, store, limits(4)).expect("fetcher"));
    let first_fetcher = Arc::clone(&fetcher);
    let first = tokio::spawn(async move {
        first_fetcher
            .fetch(MediaRequest::DirectUrl {
                url: "https://media.example/first".into(),
            })
            .await
    });
    while calls.calls().is_empty() {
        tokio::task::yield_now().await;
    }

    let second = fetcher
        .fetch(MediaRequest::DirectUrl {
            url: "https://media.example/second".into(),
        })
        .await
        .expect_err("reservation excludes concurrent fetch");
    assert!(matches!(second, FetchError::AdmissionCapacityExceeded));
    assert_eq!(calls.calls().len(), 1);

    first.abort();
    let _ = first.await;
    assert_eq!(registry.charged_bytes(), Some(0));
    let downloaded = fetcher
        .fetch(MediaRequest::DirectUrl {
            url: "https://media.example/after-cancel".into(),
        })
        .await
        .expect("released reservation is reusable");
    assert_eq!(downloaded.size_bytes, 3);
    assert_eq!(registry.charged_bytes(), Some(3));
}

#[tokio::test]
async fn commit_shrinks_charge_and_safe_discard_releases_it() {
    let root = TestRoot::new("capacity-discard");
    let registry = ManagedMediaLeaseRegistry::new_with_capacity(&root.0, 8, 4)
        .await
        .expect("registry");
    let store = pepeaudio_media::DownloadStore::new(registry.clone()).expect("store");
    let fetcher = MediaFetcher::new(
        FakeDns::public(),
        FakeHttp::new([response(b"abc")]),
        store,
        limits(8),
    )
    .expect("fetcher");
    let ingestor = MediaIngestor::new(fetcher, AcceptProbe);
    let inspected = ingestor
        .ingest(MediaRequest::DirectUrl {
            url: "https://media.example/object".into(),
        })
        .await
        .expect("ingest");
    assert_eq!(registry.charged_bytes(), Some(3));

    let lease = registry
        .acquire(&inspected.download.path)
        .await
        .expect("lease");
    assert!(ingestor.discard(&inspected.download.path).await.is_err());
    assert_eq!(registry.charged_bytes(), Some(3));
    drop(lease);
    ingestor
        .discard(&inspected.download.path)
        .await
        .expect("exclusive discard");
    assert_eq!(registry.charged_bytes(), Some(0));
}

#[tokio::test]
async fn janitor_deletion_releases_the_same_ledger() {
    let root = TestRoot::new("capacity-janitor");
    managed_file(&root.0, "objects", OBJECT_A, b"abc").await;
    let registry = ManagedMediaLeaseRegistry::new_with_capacity(&root.0, 8, 4)
        .await
        .expect("registry");
    let modified = tokio::fs::metadata(root.0.join("objects").join(OBJECT_A))
        .await
        .expect("metadata")
        .modified()
        .expect("modified");
    let policy = JanitorPolicy {
        object_ttl: Duration::from_hours(1),
        minimum_object_retention: Duration::from_mins(30),
        max_total_bytes: 8,
        max_entries_per_scan: 4,
        ..JanitorPolicy::default()
    };
    let janitor = ManagedDownloadJanitor::with_clock_and_registry(
        &root.0,
        policy,
        FixedClock(modified + Duration::from_hours(2)),
        registry.clone(),
    )
    .await
    .expect("janitor");

    janitor.run().await.expect("cleanup");
    assert_eq!(registry.charged_bytes(), Some(0));
}

#[tokio::test]
async fn on_demand_cleanup_targets_one_reservation_without_deleting_a_lease() {
    let root = TestRoot::new("capacity-on-demand");
    managed_file(&root.0, "objects", OBJECT_A, b"abc").await;
    let object = root.0.join("objects").join(OBJECT_A);
    let registry = ManagedMediaLeaseRegistry::new_with_capacity(&root.0, 8, 4)
        .await
        .expect("registry");
    let modified = tokio::fs::metadata(&object)
        .await
        .expect("metadata")
        .modified()
        .expect("modified");
    let policy = JanitorPolicy {
        minimum_object_retention: Duration::from_mins(5),
        max_total_bytes: 8,
        max_entries_per_scan: 4,
        ..JanitorPolicy::default()
    };
    let janitor = ManagedDownloadJanitor::with_clock_and_registry(
        &root.0,
        policy,
        FixedClock(modified + Duration::from_mins(6)),
        registry.clone(),
    )
    .await
    .expect("janitor");
    let lease = registry.acquire(&object).await.expect("lease");

    let protected = janitor
        .run_for_admission(6)
        .await
        .expect("protected cleanup");
    assert!(protected.removals.is_empty());
    assert!(object.exists());
    drop(lease);

    let reclaimed = janitor
        .run_for_admission(6)
        .await
        .expect("eligible cleanup");
    assert_eq!(reclaimed.removals.len(), 1);
    assert!(!object.exists());
    assert_eq!(registry.charged_bytes(), Some(0));
}

#[tokio::test]
async fn individual_download_limit_cannot_exceed_global_capacity() {
    let root = TestRoot::new("capacity-config");
    let registry = ManagedMediaLeaseRegistry::new_with_capacity(&root.0, 4, 4)
        .await
        .expect("registry");
    let store = pepeaudio_media::DownloadStore::new(registry).expect("store");
    let result = MediaFetcher::new(FakeDns::public(), FakeHttp::new([]), store, limits(5));

    assert!(matches!(
        result,
        Err(FetchError::DownloadLimitExceedsCapacity)
    ));
}
