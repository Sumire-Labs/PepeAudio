use std::{pin::Pin, time::Duration};

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::Stream;

use crate::{ApprovedUrl, FetchError, SafeHttpHeaders};

pub type ResponseBody = Pin<Box<dyn Stream<Item = Result<Bytes, BodyError>> + Send>>;

/// Sanitized body-stream failure used by HTTP fakes and adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("response body failed")]
pub struct BodyError;

pub struct HttpResponse {
    pub(super) status: u16,
    pub(super) location: Option<String>,
    pub(super) content_length: Option<u64>,
    pub(super) content_type: Option<String>,
    pub(super) body: ResponseBody,
}

impl HttpResponse {
    /// Builds a transport response. Values remain untrusted and are checked by
    /// [`crate::MediaFetcher`].
    #[must_use]
    pub fn new(
        status: u16,
        location: Option<String>,
        content_length: Option<u64>,
        content_type: Option<String>,
        body: ResponseBody,
    ) -> Self {
        Self {
            status,
            location,
            content_length,
            content_type,
            body,
        }
    }
}

/// HTTP boundary. Implementations may connect only to `target.socket_addrs()`.
#[async_trait]
pub trait HttpTransport: Send + Sync {
    /// Performs one GET with automatic redirects disabled.
    async fn get(
        &self,
        target: &ApprovedUrl,
        timeout: Duration,
        connect_timeout: Duration,
    ) -> Result<HttpResponse, FetchError>;

    /// Performs one GET with a previously validated provider-header subset.
    async fn get_with_headers(
        &self,
        target: &ApprovedUrl,
        headers: &SafeHttpHeaders,
        timeout: Duration,
        connect_timeout: Duration,
    ) -> Result<HttpResponse, FetchError> {
        let _ = headers;
        self.get(target, timeout, connect_timeout).await
    }

    /// Performs one GET with validated provider headers and an optional
    /// downloader-owned open range.
    async fn get_with_headers_and_open_range(
        &self,
        target: &ApprovedUrl,
        headers: &SafeHttpHeaders,
        use_open_range: bool,
        timeout: Duration,
        connect_timeout: Duration,
    ) -> Result<HttpResponse, FetchError> {
        let _ = use_open_range;
        self.get_with_headers(target, headers, timeout, connect_timeout)
            .await
    }
}
