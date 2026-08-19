//! Bounded, SSRF-resistant direct-media ingestion and decoding.
//!
//! Discord remains an adapter: this crate accepts transport-neutral requests
//! and does not depend on Serenity, Poise, or Songbird.

mod capacity;
mod error;
mod fetch;
mod headers;
mod ingest;
mod janitor;
mod lease;
mod model;
mod policy;
mod site;
mod store;
mod tools;

pub use capacity::ManagedMediaCapacityUsage;
pub use error::{FetchError, IngestError, ProcessError, StoreError, UrlPolicyError};
pub use fetch::{
    BodyError, HttpResponse, HttpTransport, MediaFetcher, ReqwestTransport, ResponseBody,
};
pub use headers::{SafeHeaderName, SafeHttpHeaders};
pub use ingest::{InspectedMedia, MediaIngestor, MediaProbe};
pub use janitor::{
    DEFAULT_MAX_ENTRIES_PER_SCAN, DEFAULT_MAX_TOTAL_BYTES, DEFAULT_MINIMUM_OBJECT_RETENTION,
    DEFAULT_OBJECT_TTL, DEFAULT_STAGING_TTL, JanitorClock, JanitorError, JanitorPolicy,
    JanitorRemoval, JanitorRemovalReason, JanitorReport, JanitorSkip, JanitorSkipReason,
    ManagedDownloadJanitor, SystemJanitorClock,
};
pub use lease::{ManagedMediaLease, ManagedMediaLeaseError, ManagedMediaLeaseRegistry};
pub use model::{
    DiscordAttachment, DownloadedMedia, FetchLimits, MediaRequest, MediaSourceKind, ProbeMetadata,
    ProbeStream, ResolvedSiteMedia,
};
pub use policy::{ApprovedUrl, DnsResolver, TokioDnsResolver, UrlGuard, is_forbidden_ip};
pub use site::{
    SiteCollection, SiteError, SiteProvider, SiteReference, SiteResolvedTrack, SiteSearch,
    YtDlpClient, YtDlpConfig,
};
pub use store::DownloadStore;
pub use tools::{
    CommandSpec, DecodeExit, DecoderSpawner, Ffmpeg, FfmpegDecoder, Ffprobe, OutputLimits,
    PcmDecoder, ProcessOutput, ProcessPool, ProcessRunner, RealProcessRunner,
};
