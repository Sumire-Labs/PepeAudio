#![allow(dead_code)]

use std::{
    collections::{HashMap, VecDeque},
    net::IpAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use pepeaudio_media::{
    ApprovedUrl, DnsResolver, DownloadStore, FetchError, HttpResponse, HttpTransport,
    ManagedMediaLeaseRegistry, UrlPolicyError,
};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(crate) struct FakeDns {
    default: Vec<IpAddr>,
    answers: Arc<HashMap<String, Vec<IpAddr>>>,
    calls: Arc<Mutex<Vec<String>>>,
}

impl FakeDns {
    pub(crate) fn public() -> Self {
        Self::new(vec![IpAddr::from([93, 184, 216, 34])])
    }

    pub(crate) fn new(default: Vec<IpAddr>) -> Self {
        Self {
            default,
            answers: Arc::new(HashMap::new()),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn with_answers(mut self, answers: HashMap<String, Vec<IpAddr>>) -> Self {
        self.answers = Arc::new(answers);
        self
    }

    pub(crate) fn call_count(&self) -> usize {
        self.calls.lock().expect("call lock").len()
    }
}

#[async_trait]
impl DnsResolver for FakeDns {
    async fn resolve(&self, domain: &str) -> Result<Vec<IpAddr>, UrlPolicyError> {
        self.calls
            .lock()
            .expect("call lock")
            .push(domain.to_owned());
        Ok(self
            .answers
            .get(domain)
            .cloned()
            .unwrap_or_else(|| self.default.clone()))
    }
}

#[derive(Clone)]
pub(crate) struct FakeHttp {
    responses: Arc<Mutex<VecDeque<HttpResponse>>>,
    calls: Arc<Mutex<Vec<String>>>,
}

impl FakeHttp {
    pub(crate) fn new(responses: impl IntoIterator<Item = HttpResponse>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into_iter().collect())),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("call lock").clone()
    }
}

#[async_trait]
impl HttpTransport for FakeHttp {
    async fn get(
        &self,
        target: &ApprovedUrl,
        _timeout: std::time::Duration,
        _connect_timeout: std::time::Duration,
    ) -> Result<HttpResponse, FetchError> {
        self.calls
            .lock()
            .expect("call lock")
            .push(target.url().as_str().to_owned());
        self.responses
            .lock()
            .expect("responses lock")
            .pop_front()
            .ok_or(FetchError::Transport)
    }
}

pub(crate) struct TestRoot(pub(crate) PathBuf);

impl TestRoot {
    pub(crate) fn new(label: &str) -> Self {
        Self(std::env::temp_dir().join(format!(
            "pepeaudio-media-{label}-{}",
            Uuid::new_v4().simple()
        )))
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) async fn download_store(root: &TestRoot) -> DownloadStore {
    let registry = ManagedMediaLeaseRegistry::new_with_capacity(&root.0, 1024 * 1024 * 1024, 4096)
        .await
        .expect("capacity registry");
    DownloadStore::new(registry).expect("metered download store")
}
