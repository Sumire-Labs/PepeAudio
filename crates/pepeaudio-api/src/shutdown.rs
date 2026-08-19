use tokio::sync::watch;

/// Process-local signal used to finish long-lived API response streams.
///
/// The HTTP server still owns its transport-level graceful shutdown. This
/// signal lets streaming handlers release their connections promptly instead
/// of making that transport drain wait for the normal SSE lease expiry.
#[derive(Clone, Debug)]
pub struct ApiShutdown {
    sender: watch::Sender<bool>,
}

impl ApiShutdown {
    #[must_use]
    pub fn new() -> Self {
        let (sender, _receiver) = watch::channel(false);
        Self { sender }
    }

    pub fn trigger(&self) {
        self.sender.send_replace(true);
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<bool> {
        self.sender.subscribe()
    }
}

impl Default for ApiShutdown {
    fn default() -> Self {
        Self::new()
    }
}
