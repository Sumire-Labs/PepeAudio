use pepeaudio_components_v2::Message;
use poise::serenity_prelude as serenity;
use thiserror::Error;

use crate::{CommandError, Context};

pub(crate) async fn edit_deferred(
    ctx: Context<'_>,
    payload: &Message,
) -> Result<serenity::MessageId, CommandError> {
    let poise::Context::Application(application) = ctx else {
        return Err("a Components V2 response requires an application command".into());
    };
    ctx.data()
        .components
        .edit_original_response(
            application.interaction.application_id,
            &application.interaction.token,
            payload,
        )
        .await
        .map_err(Into::into)
}

pub(crate) async fn edit_deferred_after_apply(
    ctx: Context<'_>,
    payload: &Message,
) -> Result<(), CommandError> {
    edit_deferred(ctx, payload)
        .await
        .map(|_message_id| ())
        .map_err(applied_response_error)
}

pub(crate) fn applied_response_error<E>(source: E) -> CommandError
where
    E: Into<CommandError>,
{
    Box::new(AppliedResponseError {
        source: source.into(),
    })
}

#[derive(Debug, Error)]
#[error("the player mutation succeeded, but its Discord response could not be updated")]
pub(crate) struct AppliedResponseError {
    #[source]
    source: CommandError,
}

#[cfg(test)]
mod tests {
    use super::{AppliedResponseError, applied_response_error};

    #[test]
    fn applied_response_failures_keep_a_distinct_public_outcome() {
        let error = applied_response_error(std::io::Error::other("test response failure"));
        assert!(error.downcast_ref::<AppliedResponseError>().is_some());
        assert!(!error.to_string().contains("test response failure"));
    }
}
