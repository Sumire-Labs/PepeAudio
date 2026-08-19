use std::{io, time::Duration};

/// Rejection produced before a network connection is allowed.
#[derive(Debug, thiserror::Error)]
pub enum UrlPolicyError {
    #[error("URL exceeds the configured {max_bytes}-byte limit")]
    TooLong { max_bytes: usize },
    #[error("URL is malformed")]
    Malformed,
    #[error("URL scheme is not http or https")]
    UnsupportedScheme,
    #[error("URL user information is forbidden")]
    UserInfo,
    /// Fragments are not sent to servers and are forbidden for canonicality.
    #[error("URL fragments are forbidden")]
    Fragment,
    #[error("URL host or port is missing")]
    MissingAuthority,
    #[error("DNS resolution returned no addresses")]
    EmptyDnsAnswer,
    /// An unexpectedly large DNS answer is rejected to bound work and memory.
    #[error("DNS resolution returned too many addresses")]
    TooManyDnsAnswers,
    #[error("URL resolves to a forbidden network range")]
    ForbiddenAddress,
    /// DNS resolution failed. The hostname and resolver detail are omitted.
    #[error("DNS resolution failed")]
    Dns,
    #[error("DNS resolution exceeded {0:?}")]
    DnsTimeout(Duration),
    /// Secure redirects may not silently downgrade their transport.
    #[error("redirect from https to http is forbidden")]
    InsecureRedirect,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("managed media storage capacity is exhausted")]
    CapacityExceeded,
    #[error("managed media storage has no hard capacity configured")]
    CapacityUnavailable,
    #[error("managed media capacity accounting failed closed")]
    Accounting,
    #[error("managed media discard target is unsafe")]
    UnsafeObject,
    #[error("managed media object is currently in use")]
    ObjectInUse,
    #[error("could not prepare managed media storage")]
    Prepare(#[source] io::Error),
    #[error("could not allocate a managed partial file")]
    Allocate(#[source] io::Error),
    #[error("could not write the managed partial file")]
    Write(#[source] io::Error),
    #[error("could not commit the managed media file")]
    Commit(#[source] io::Error),
    #[error("could not clean up a managed media file")]
    Cleanup(#[source] io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error(transparent)]
    Url(#[from] UrlPolicyError),
    #[error("fetch limits must be non-zero")]
    InvalidLimits,
    #[error("managed media capacity is exhausted before network access")]
    AdmissionCapacityExceeded,
    #[error("per-download byte limit exceeds managed media capacity")]
    DownloadLimitExceedsCapacity,
    #[error("declared attachment size exceeds the configured byte limit")]
    DeclaredSizeTooLarge,
    #[error("redirect response has no valid Location header")]
    MissingLocation,
    #[error("redirect limit exceeded")]
    RedirectLimit,
    #[error("redirect loop detected")]
    RedirectLoop,
    #[error("resolved provider media redirected to an unapproved host")]
    UnapprovedSiteHost,
    #[error("remote server returned HTTP status {0}")]
    HttpStatus(u16),
    #[error("Content-Length exceeds the configured byte limit")]
    ContentLengthTooLarge,
    #[error("download exceeded the configured byte limit")]
    DownloadTooLarge,
    #[error("remote media response was empty")]
    EmptyBody,
    #[error("download length did not match Content-Length")]
    LengthMismatch,
    #[error("redirect and response headers exceeded their time limit")]
    RedirectTimeout,
    #[error("media download exceeded its time limit")]
    DownloadTimeout,
    /// HTTP adapter failed without exposing response contents.
    #[error("HTTP transport failed")]
    Transport,
    #[error("HTTP response body failed")]
    Body,
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("media tool limits must be non-zero")]
    InvalidConfig,
    #[error("media tool could not be spawned")]
    Spawn(#[source] io::Error),
    #[error("media tool pipe failed")]
    Pipe(#[source] io::Error),
    #[error("media tool exceeded its time limit")]
    Timeout,
    #[error("media tool output exceeded its configured limit")]
    OutputTooLarge,
    /// Tool exited unsuccessfully. Output text is intentionally omitted.
    #[error("media tool exited unsuccessfully with code {code:?}")]
    Exit { code: Option<i32> },
    /// `yt-dlp` identified one requested item as unavailable. The diagnostic
    /// text and page URL are intentionally omitted.
    #[error("requested media item is unavailable")]
    MediaUnavailable,
    #[error("ffprobe returned invalid metadata")]
    InvalidProbe,
    #[error("media contains no audio stream")]
    NoAudioStream,
    #[error("ffmpeg decoder did not expose PCM stdout")]
    MissingStdout,
    #[error("ffmpeg decoder lifecycle operation failed")]
    Lifecycle(#[source] io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error(transparent)]
    Fetch(#[from] FetchError),
    #[error(transparent)]
    Probe(#[from] ProcessError),
    #[error("could not safely remove rejected managed media")]
    Cleanup(#[source] StoreError),
}
