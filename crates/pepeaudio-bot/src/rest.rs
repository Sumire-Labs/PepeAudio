use async_trait::async_trait;
use pepeaudio_components_v2::Message;
use serenity::{
    http::{Http, LightMethod, Request, Route},
    model::id::{ApplicationId, ChannelId, InteractionId},
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
    ) -> Result<(), RestBoundaryError>;

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
    ) -> Result<(), RestBoundaryError> {
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
        self.http
            .request(request)
            .await
            .map(|_response| ())
            .map_err(|error| RestBoundaryError::Discord(error.to_string()))
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
}
