use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::{Client, Method, header::CONTENT_LENGTH, redirect::Policy};
use url::Url;

const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HttpError {
    Transport,
    ResponseTooLarge,
}

pub(crate) struct HttpRequest {
    method: Method,
    url: Url,
    headers: Vec<(&'static str, String)>,
    body: Option<Vec<u8>>,
}

impl HttpRequest {
    pub(crate) fn get(url: Url) -> Self {
        Self {
            method: Method::GET,
            url,
            headers: Vec::new(),
            body: None,
        }
    }

    pub(crate) fn post_form(url: Url, body: Vec<u8>) -> Self {
        Self {
            method: Method::POST,
            url,
            headers: vec![(
                "content-type",
                "application/x-www-form-urlencoded".to_owned(),
            )],
            body: Some(body),
        }
    }

    pub(crate) fn with_header(mut self, name: &'static str, value: String) -> Self {
        self.headers.push((name, value));
        self
    }

    #[cfg(test)]
    pub(crate) fn url(&self) -> &Url {
        &self.url
    }

    #[cfg(test)]
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

pub(crate) struct HttpResponse {
    pub(crate) status: u16,
    pub(crate) retry_after_seconds: Option<u64>,
    pub(crate) body: Vec<u8>,
}

impl HttpResponse {
    #[cfg(test)]
    pub(crate) const fn new(status: u16, body: Vec<u8>) -> Self {
        Self {
            status,
            retry_after_seconds: None,
            body,
        }
    }
}

#[async_trait]
pub(crate) trait HttpTransport: Send + Sync {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, HttpError>;
}

pub(crate) type SharedTransport = Arc<dyn HttpTransport>;

#[derive(Clone)]
pub(crate) struct ReqwestTransport {
    client: Client,
}

impl ReqwestTransport {
    pub(crate) fn new() -> Result<Self, HttpError> {
        let client = Client::builder()
            .redirect(Policy::none())
            .no_proxy()
            .referer(false)
            .retry(reqwest::retry::never())
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|_| HttpError::Transport)?;
        Ok(Self { client })
    }
}

#[async_trait]
impl HttpTransport for ReqwestTransport {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, HttpError> {
        let mut builder = self.client.request(request.method, request.url);
        for (name, value) in request.headers {
            builder = builder.header(name, value);
        }
        if let Some(body) = request.body {
            builder = builder.body(body);
        }
        let response = builder.send().await.map_err(|_| HttpError::Transport)?;
        if response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
        {
            return Err(HttpError::ResponseTooLarge);
        }
        let status = response.status().as_u16();
        let retry_after_seconds = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok());
        let mut stream = response.bytes_stream();
        let mut body = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| HttpError::Transport)?;
            let next_length = body
                .len()
                .checked_add(chunk.len())
                .ok_or(HttpError::ResponseTooLarge)?;
            if next_length > MAX_RESPONSE_BYTES {
                return Err(HttpError::ResponseTooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        Ok(HttpResponse {
            status,
            retry_after_seconds,
            body,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use super::*;

    pub(crate) struct ScriptedTransport {
        responses: Mutex<VecDeque<Result<HttpResponse, HttpError>>>,
        requests: Mutex<Vec<HttpRequest>>,
    }

    impl ScriptedTransport {
        pub(crate) fn new(responses: impl IntoIterator<Item = HttpResponse>) -> Arc<Self> {
            Arc::new(Self {
                responses: Mutex::new(responses.into_iter().map(Ok).collect()),
                requests: Mutex::new(Vec::new()),
            })
        }

        pub(crate) fn request_urls(&self) -> Vec<String> {
            self.requests
                .lock()
                .expect("requests lock")
                .iter()
                .map(|request| request.url().to_string())
                .collect()
        }

        pub(crate) fn request_header(&self, index: usize, name: &str) -> Option<String> {
            self.requests
                .lock()
                .expect("requests lock")
                .get(index)
                .and_then(|request| request.header(name))
                .map(str::to_owned)
        }
    }

    #[async_trait]
    impl HttpTransport for ScriptedTransport {
        async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, HttpError> {
            self.requests.lock().expect("requests lock").push(request);
            self.responses
                .lock()
                .expect("responses lock")
                .pop_front()
                .expect("scripted response")
        }
    }
}
