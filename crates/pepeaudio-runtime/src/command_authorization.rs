use async_trait::async_trait;
use pepeaudio_core::CommandEnvelope;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandAuthorization {
    Allowed,
    Denied,
    RetryableFailure,
}

/// Implementations run in the process that owns the target Discord shard. They
/// must fail closed: unavailable cache, gateway, or policy dependencies return
/// [`CommandAuthorization::RetryableFailure`], never `Allowed`.
#[async_trait]
pub trait CommandAuthorizer: Send + Sync + 'static {
    async fn authorize(&self, command: &CommandEnvelope) -> CommandAuthorization;
}
