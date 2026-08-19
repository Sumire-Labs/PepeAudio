use std::{io, net::SocketAddr, sync::Arc, time::Duration};

use async_trait::async_trait;
use futures_util::{StreamExt, TryStreamExt};
use reqwest::{
    Client,
    dns::{Addrs, Name, Resolve, Resolving},
    header::{
        ACCEPT, ACCEPT_LANGUAGE, CONTENT_LENGTH, CONTENT_TYPE, HeaderMap, HeaderValue, LOCATION,
        ORIGIN, REFERER, USER_AGENT,
    },
    redirect::Policy,
};

use super::{BodyError, HttpResponse, HttpTransport};
use crate::{ApprovedUrl, FetchError, SafeHeaderName, SafeHttpHeaders};

/// Reqwest adapter with per-hop DNS pinning and no proxy or auto-redirects.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReqwestTransport;

#[async_trait]
impl HttpTransport for ReqwestTransport {
    async fn get(
        &self,
        target: &ApprovedUrl,
        timeout: Duration,
        connect_timeout: Duration,
    ) -> Result<HttpResponse, FetchError> {
        self.get_with_headers(
            target,
            &SafeHttpHeaders::default(),
            timeout,
            connect_timeout,
        )
        .await
    }

    async fn get_with_headers(
        &self,
        target: &ApprovedUrl,
        headers: &SafeHttpHeaders,
        timeout: Duration,
        connect_timeout: Duration,
    ) -> Result<HttpResponse, FetchError> {
        let resolver = PinnedResolver {
            expected_host: target
                .url()
                .host_str()
                .ok_or(FetchError::Transport)?
                .to_owned(),
            addresses: target.socket_addrs().to_vec(),
        };
        let client = build_client(resolver, timeout, connect_timeout)?;
        let response = client
            .get(target.url().clone())
            .headers(reqwest_headers(headers)?)
            .send()
            .await
            .map_err(|_| FetchError::Transport)?;
        let status = response.status().as_u16();
        let location = optional_header(response.headers(), LOCATION)?;
        let content_type =
            optional_header(response.headers(), CONTENT_TYPE)?.filter(|value| value.len() <= 256);
        let content_length = content_length(response.headers())?;
        let body = response.bytes_stream().map_err(|_| BodyError).boxed();
        Ok(HttpResponse::new(
            status,
            location,
            content_length,
            content_type,
            body,
        ))
    }
}

fn reqwest_headers(headers: &SafeHttpHeaders) -> Result<HeaderMap, FetchError> {
    let mut output = HeaderMap::new();
    for (name, value) in headers.iter() {
        let name = match name {
            SafeHeaderName::UserAgent => USER_AGENT,
            SafeHeaderName::Referer => REFERER,
            SafeHeaderName::Origin => ORIGIN,
            SafeHeaderName::Accept => ACCEPT,
            SafeHeaderName::AcceptLanguage => ACCEPT_LANGUAGE,
        };
        let value = HeaderValue::from_str(value).map_err(|_| FetchError::Transport)?;
        if output.insert(name, value).is_some() {
            return Err(FetchError::Transport);
        }
    }
    Ok(output)
}

fn build_client(
    resolver: PinnedResolver,
    timeout: Duration,
    connect_timeout: Duration,
) -> Result<Client, FetchError> {
    Client::builder()
        .dns_resolver(Arc::new(resolver))
        .redirect(Policy::none())
        .no_proxy()
        .referer(false)
        .retry(reqwest::retry::never())
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .connect_timeout(connect_timeout.min(timeout))
        .timeout(timeout)
        .build()
        .map_err(|_| FetchError::Transport)
}

fn optional_header(
    headers: &reqwest::header::HeaderMap,
    name: reqwest::header::HeaderName,
) -> Result<Option<String>, FetchError> {
    headers
        .get(name)
        .map(|value| {
            value
                .to_str()
                .map(str::to_owned)
                .map_err(|_| FetchError::Transport)
        })
        .transpose()
}

fn content_length(headers: &reqwest::header::HeaderMap) -> Result<Option<u64>, FetchError> {
    let mut values = headers.get_all(CONTENT_LENGTH).iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(FetchError::Transport);
    }
    value
        .to_str()
        .ok()
        .and_then(|text| text.parse::<u64>().ok())
        .map(Some)
        .ok_or(FetchError::Transport)
}

#[derive(Clone, Debug)]
struct PinnedResolver {
    expected_host: String,
    addresses: Vec<SocketAddr>,
}

impl Resolve for PinnedResolver {
    fn resolve(&self, name: Name) -> Resolving {
        if !name.as_str().eq_ignore_ascii_case(&self.expected_host) {
            let error = io::Error::new(io::ErrorKind::PermissionDenied, "unexpected DNS target");
            return Box::pin(async move { Err(Box::new(error).into()) });
        }
        let addresses: Addrs = Box::new(self.addresses.clone().into_iter());
        Box::pin(async move { Ok(addresses) })
    }
}

#[cfg(test)]
#[path = "reqwest_tests.rs"]
mod tests;
