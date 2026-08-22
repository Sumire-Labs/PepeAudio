use async_trait::async_trait;
use pepeaudio_components_v2::Message;
use serenity::{
    http::{Http, LightMethod, Request, Route},
    model::id::{ApplicationId, ChannelId, InteractionId, MessageId},
};
use thiserror::Error;

/// Stable-Serenity isolation boundary for raw Components V2 responses.
#[async_trait]
pub trait ComponentsV2Responder: Send + Sync + 'static {
    async fn create_response(
        &self,
        interaction_id: InteractionId,
        interaction_token: &str,
        payload: &Message,
    ) -> Result<(), RestBoundaryError>;

    async fn edit_original_response(
        &self,
        application_id: ApplicationId,
        interaction_token: &str,
        payload: &Message,
    ) -> Result<MessageId, RestBoundaryError>;

    async fn edit_message(
        &self,
        channel_id: ChannelId,
        message_id: serenity::model::id::MessageId,
        payload: &Message,
    ) -> Result<(), RestBoundaryError>;
}

/// Raw REST adapter reserved for the stable Serenity Components V2 gap.
pub struct DiscordComponentsV2Rest {
    http: std::sync::Arc<Http>,
}

impl DiscordComponentsV2Rest {
    #[must_use]
    pub const fn new(http: std::sync::Arc<Http>) -> Self {
        Self { http }
    }
}

#[async_trait]
impl ComponentsV2Responder for DiscordComponentsV2Rest {
    async fn create_response(
        &self,
        interaction_id: InteractionId,
        interaction_token: &str,
        payload: &Message,
    ) -> Result<(), RestBoundaryError> {
        payload.validate().map_err(RestBoundaryError::Payload)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "type": 4,
            "data": payload,
        }))
        .map_err(RestBoundaryError::Serialization)?;
        let request = Request::new(
            Route::InteractionResponse {
                interaction_id,
                token: interaction_token,
            },
            LightMethod::Post,
        )
        .body(Some(body));
        self.http
            .request(request)
            .await
            .map(|_response| ())
            .map_err(|error| RestBoundaryError::Discord(error.to_string()))
    }

    async fn edit_original_response(
        &self,
        application_id: ApplicationId,
        interaction_token: &str,
        payload: &Message,
    ) -> Result<MessageId, RestBoundaryError> {
        payload.validate().map_err(RestBoundaryError::Payload)?;
        let body = serde_json::to_vec(payload).map_err(RestBoundaryError::Serialization)?;
        let request = Request::new(
            Route::WebhookOriginalInteractionResponse {
                application_id,
                token: interaction_token,
            },
            LightMethod::Patch,
        )
        .body(Some(body));
        let response = self
            .http
            .request(request)
            .await
            .map_err(|error| RestBoundaryError::Discord(error.to_string()))?;
        let body = response
            .bytes()
            .await
            .map_err(|error| RestBoundaryError::Discord(error.to_string()))?;
        message_id_from_response(&body)
    }

    async fn edit_message(
        &self,
        channel_id: ChannelId,
        message_id: serenity::model::id::MessageId,
        payload: &Message,
    ) -> Result<(), RestBoundaryError> {
        payload.validate().map_err(RestBoundaryError::Payload)?;
        let body = serde_json::to_vec(payload).map_err(RestBoundaryError::Serialization)?;
        let request = Request::new(
            Route::ChannelMessage {
                channel_id,
                message_id,
            },
            LightMethod::Patch,
        )
        .body(Some(body));
        self.http
            .request(request)
            .await
            .map(|_response| ())
            .map_err(|error| RestBoundaryError::Discord(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum RestBoundaryError {
    #[error(transparent)]
    Payload(#[from] pepeaudio_components_v2::ValidationError),
    #[error("failed to serialize a validated Components V2 payload: {0}")]
    Serialization(serde_json::Error),
    #[error("Discord REST request failed: {0}")]
    Discord(String),
    #[error("Discord returned an invalid message identity")]
    InvalidMessageIdentity,
}

fn message_id_from_response(body: &[u8]) -> Result<MessageId, RestBoundaryError> {
    let response: serde_json::Value =
        serde_json::from_slice(body).map_err(|_| RestBoundaryError::InvalidMessageIdentity)?;
    let raw = response
        .get("id")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| value.parse::<std::num::NonZeroU64>().ok())
        .ok_or(RestBoundaryError::InvalidMessageIdentity)?;
    Ok(MessageId::new(raw.get()))
}

#[cfg(test)]
mod tests {
    use super::{RestBoundaryError, message_id_from_response};

    #[test]
    fn reads_decimal_string_message_identity() {
        let message = message_id_from_response(br#"{"id":"123"}"#).expect("message id");

        assert_eq!(message.get(), 123);
    }

    #[test]
    fn rejects_missing_numeric_and_zero_message_identities() {
        for body in [
            br"{}".as_slice(),
            br#"{"id":123}"#.as_slice(),
            br#"{"id":"0"}"#.as_slice(),
        ] {
            assert!(matches!(
                message_id_from_response(body),
                Err(RestBoundaryError::InvalidMessageIdentity)
            ));
        }
    }
}
