use std::collections::HashSet;

use futures_util::StreamExt;
use tokio::time::{Instant, timeout_at};

use super::transport::{HttpResponse, HttpTransport};
use crate::{
    DnsResolver, DownloadStore, DownloadedMedia, FetchError, FetchLimits, MediaRequest, UrlGuard,
};

/// Redirect-aware, bounded downloader for direct URLs and attachments.
#[derive(Debug)]
pub struct MediaFetcher<R, T> {
    resolver: R,
    transport: T,
    store: DownloadStore,
    limits: FetchLimits,
    guard: UrlGuard,
}

impl<R, T> MediaFetcher<R, T>
where
    R: DnsResolver,
    T: HttpTransport,
{
    /// # Errors
    ///
    /// Returns [`FetchError::InvalidLimits`] for a zero hard limit, or
    /// [`FetchError::DownloadLimitExceedsCapacity`] when one download could
    /// exceed the entire managed-media budget.
    pub fn new(
        resolver: R,
        transport: T,
        store: DownloadStore,
        limits: FetchLimits,
    ) -> Result<Self, FetchError> {
        validate_limits(limits)?;
        if limits.max_download_bytes > store.maximum_bytes() {
            return Err(FetchError::DownloadLimitExceedsCapacity);
        }
        Ok(Self {
            resolver,
            transport,
            store,
            limits,
            guard: UrlGuard::new(limits.max_url_bytes, limits.dns_timeout),
        })
    }

    /// Fetches a remote object through the same policy path for both sources.
    /// Partial files are removed on failure.
    ///
    /// # Errors
    ///
    /// Returns [`FetchError`] when URL policy, transport, resource limits, or
    /// managed storage reject the object.
    pub async fn fetch(&self, request: MediaRequest) -> Result<DownloadedMedia, FetchError> {
        if request
            .declared_size()
            .is_some_and(|size| size > self.limits.max_download_bytes)
        {
            return Err(FetchError::DeclaredSizeTooLarge);
        }

        // Reserve before URL parsing, DNS, or HTTP so a full process-private
        // root cannot be amplified into outbound network work. Unknown direct
        // downloads reserve the full individual cap; declared attachments may
        // grow their reservation before any corresponding bytes are written.
        let initial_reservation = request
            .declared_size()
            .unwrap_or(self.limits.max_download_bytes)
            .max(1);
        let mut reservation =
            self.store
                .reserve(initial_reservation)
                .map_err(|error| match error {
                    crate::StoreError::CapacityExceeded => FetchError::AdmissionCapacityExceeded,
                    other => FetchError::Store(other),
                })?;

        let source_kind = request.source_kind();
        let header_deadline = Instant::now()
            .checked_add(self.limits.redirect_timeout)
            .ok_or(FetchError::InvalidLimits)?;
        let (response, final_url) = self.follow_redirects(&request, header_deadline).await?;
        if response
            .content_length
            .is_some_and(|size| size > self.limits.max_download_bytes)
        {
            return Err(FetchError::ContentLengthTooLarge);
        }
        if let Some(content_length) = response.content_length {
            reservation.ensure(content_length.max(1))?;
        }

        let content_length = response.content_length;
        let content_type = response.content_type;
        let mut body = response.body;
        let download_deadline = Instant::now()
            .checked_add(self.limits.download_timeout)
            .ok_or(FetchError::InvalidLimits)?;
        let mut partial = timeout_at(download_deadline, self.store.begin(reservation))
            .await
            .map_err(|_| FetchError::DownloadTimeout)??;
        let mut size_bytes = 0_u64;

        let download = async {
            loop {
                let next = timeout_at(download_deadline, body.next())
                    .await
                    .map_err(|_| FetchError::DownloadTimeout)?;
                let Some(chunk) = next else { break };
                let chunk = chunk.map_err(|_| FetchError::Body)?;
                let chunk_len =
                    u64::try_from(chunk.len()).map_err(|_| FetchError::DownloadTooLarge)?;
                size_bytes = size_bytes
                    .checked_add(chunk_len)
                    .ok_or(FetchError::DownloadTooLarge)?;
                if size_bytes > self.limits.max_download_bytes {
                    return Err(FetchError::DownloadTooLarge);
                }
                partial.ensure_reserved(size_bytes.max(1))?;
                timeout_at(download_deadline, partial.write_all(&chunk))
                    .await
                    .map_err(|_| FetchError::DownloadTimeout)??;
            }
            if size_bytes == 0 {
                return Err(FetchError::EmptyBody);
            }
            if content_length.is_some_and(|expected| expected != size_bytes) {
                return Err(FetchError::LengthMismatch);
            }
            timeout_at(download_deadline, partial.commit(size_bytes))
                .await
                .map_err(|_| FetchError::DownloadTimeout)?
                .map_err(FetchError::from)
        }
        .await;

        match download {
            Ok(path) => Ok(DownloadedMedia {
                path,
                final_url,
                size_bytes,
                content_type,
                source_kind,
            }),
            Err(error) => {
                partial.cleanup().await?;
                Err(error)
            }
        }
    }

    pub(crate) async fn discard(&self, path: &std::path::Path) -> Result<(), crate::StoreError> {
        self.store.discard_object(path).await
    }

    async fn follow_redirects(
        &self,
        request: &MediaRequest,
        deadline: Instant,
    ) -> Result<(HttpResponse, String), FetchError> {
        let mut current = request.url().to_owned();
        let mut visited = HashSet::new();
        let mut redirect_count = 0_usize;

        loop {
            let approved = timeout_at(deadline, self.guard.approve(&current, &self.resolver))
                .await
                .map_err(|_| FetchError::RedirectTimeout)??;
            let host = approved.url().host_str().ok_or(FetchError::Transport)?;
            if !request.allows_host(host) {
                return Err(FetchError::UnapprovedSiteHost);
            }
            let canonical = approved.url().as_str().to_owned();
            if !visited.insert(canonical.clone()) {
                return Err(FetchError::RedirectLoop);
            }
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or(FetchError::RedirectTimeout)?;
            let response = timeout_at(
                deadline,
                self.transport.get_with_headers_and_open_range(
                    &approved,
                    request.headers(),
                    request.uses_open_range(),
                    remaining,
                    self.limits.connect_timeout,
                ),
            )
            .await
            .map_err(|_| FetchError::RedirectTimeout)??;

            if is_redirect(response.status) {
                if redirect_count >= self.limits.max_redirects {
                    return Err(FetchError::RedirectLimit);
                }
                let location = response.location.ok_or(FetchError::MissingLocation)?;
                current = self.guard.join(approved.url(), &location)?;
                redirect_count += 1;
                continue;
            }
            if !(200..300).contains(&response.status) {
                return Err(FetchError::HttpStatus(response.status));
            }
            return Ok((response, canonical));
        }
    }
}

const fn is_redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

fn validate_limits(limits: FetchLimits) -> Result<(), FetchError> {
    if limits.max_url_bytes == 0
        || limits.max_download_bytes == 0
        || limits.redirect_timeout.is_zero()
        || limits.download_timeout.is_zero()
        || limits.dns_timeout.is_zero()
        || limits.connect_timeout.is_zero()
    {
        return Err(FetchError::InvalidLimits);
    }
    Ok(())
}
