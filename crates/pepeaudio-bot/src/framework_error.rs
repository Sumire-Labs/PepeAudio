use pepeaudio_components_v2::{Component, Message};
use poise::serenity_prelude as serenity;

use crate::{BotData, CommandError};

pub(crate) fn on_error(
    error: poise::FrameworkError<'_, BotData, CommandError>,
) -> poise::BoxFuture<'_, ()> {
    Box::pin(async move {
        let (error_class, mutation_was_applied) = failure_class(&error);
        let Some(ctx) = error.ctx() else {
            tracing::error!(
                error_class,
                "Discord framework operation failed without command context"
            );
            return;
        };
        let poise::Context::Application(application) = ctx else {
            tracing::warn!(
                guild_id = ?ctx.guild_id().map(serenity::GuildId::get),
                command = %ctx.command().qualified_name,
                error_class,
                mutation_was_applied,
                "Discord command failed outside an application interaction"
            );
            return;
        };
        let interaction = &application.interaction;
        tracing::warn!(
            guild_id = ?ctx.guild_id().map(serenity::GuildId::get),
            command = %ctx.command().qualified_name,
            interaction_id = interaction.id.get(),
            error_class,
            mutation_was_applied,
            "Discord application command failed"
        );
        let public_message = public_failure_message(&error, mutation_was_applied);
        let components = || vec![Component::container(vec![Component::text(public_message)])];
        let Ok(edit_payload) = Message::new(components()) else {
            log_response_failure(ctx, interaction.id.get(), mutation_was_applied, "payload");
            return;
        };
        if ctx
            .data()
            .components
            .edit_original_response(
                interaction.application_id,
                &interaction.token,
                &edit_payload,
            )
            .await
            .is_err()
        {
            log_response_failure(
                ctx,
                interaction.id.get(),
                mutation_was_applied,
                "edit_original",
            );
            let Ok(create_payload) = Message::ephemeral(components()) else {
                log_response_failure(
                    ctx,
                    interaction.id.get(),
                    mutation_was_applied,
                    "fallback_payload",
                );
                return;
            };
            if ctx
                .data()
                .components
                .create_response(interaction.id, &interaction.token, &create_payload)
                .await
                .is_err()
            {
                log_response_failure(
                    ctx,
                    interaction.id.get(),
                    mutation_was_applied,
                    "fallback_create",
                );
            }
        }
    })
}

fn public_failure_message(
    error: &poise::FrameworkError<'_, BotData, CommandError>,
    mutation_was_applied: bool,
) -> &'static str {
    if mutation_was_applied {
        return "操作は反映されましたが、応答を更新できませんでした。`/now`を開き直してください。";
    }
    let poise::FrameworkError::Command { error, .. } = error else {
        return "操作を完了できませんでした。入力内容、ボイスチャンネル、権限を確認して再試行してください。";
    };
    if let Some(error) = error.downcast_ref::<crate::commands::PlayInputError>() {
        return play_input_failure_message(error);
    }
    error
        .downcast_ref::<crate::ResolveError>()
        .and_then(media_failure_message)
        .unwrap_or(
            "操作を完了できませんでした。入力内容、ボイスチャンネル、権限を確認して再試行してください。",
        )
}

fn play_input_failure_message(error: &crate::commands::PlayInputError) -> &'static str {
    match error {
        crate::commands::PlayInputError::Missing => {
            "URLまたは音声ファイルのどちらか一方を指定してください。"
        }
        crate::commands::PlayInputError::Conflicting => {
            "URLと音声ファイルは同時に指定できません。どちらか一方を選んでください。"
        }
    }
}

fn media_failure_message(error: &crate::ResolveError) -> Option<&'static str> {
    match error {
        crate::ResolveError::NoSearchMatch => {
            Some("安全に一致するYouTubeまたはSoundCloud音源を確認できませんでした。")
        }
        crate::ResolveError::UnsupportedStream => Some(
            "このURLから再生可能な音源を取得できませんでした。動画が公開中で、地域・年齢制限がないか確認してください。",
        ),
        crate::ResolveError::SpotifyPlaylistRequiresUserAuthorization => Some(
            "Spotifyプレイリストの取り込みにはSpotifyユーザー認証が必要なため、現在は利用できません。",
        ),
        crate::ResolveError::SpotifyAlbumRequiresCredentials => Some(
            "Spotifyアルバムの取り込みには、運用側でSpotify credential overlayの有効化が必要です。",
        ),
        crate::ResolveError::AppleMusicPlaylistRequiresDeveloperCredentials => Some(
            "Apple Musicプレイリストの取り込みにはApple Developerの認証情報が必要です。曲とアルバムのリンクは認証情報なしで利用できます。",
        ),
        crate::ResolveError::CrossServiceMatchingDisabled => {
            Some("Spotify・Apple Musicリンクからの音源照合は、このBotでは無効です。")
        }
        _ => None,
    }
}

fn log_response_failure(
    ctx: poise::Context<'_, BotData, CommandError>,
    interaction_id: u64,
    mutation_was_applied: bool,
    response_stage: &'static str,
) {
    tracing::warn!(
        guild_id = ?ctx.guild_id().map(serenity::GuildId::get),
        command = %ctx.command().qualified_name,
        interaction_id,
        mutation_was_applied,
        error_class = "public_response",
        response_stage,
        "Discord command failure response could not be delivered"
    );
}

fn failure_class(error: &poise::FrameworkError<'_, BotData, CommandError>) -> (&'static str, bool) {
    match error {
        poise::FrameworkError::Command { error, .. } => {
            let applied = error
                .downcast_ref::<crate::commands::response::AppliedResponseError>()
                .is_some();
            (command_error_class(error), applied)
        }
        poise::FrameworkError::CommandPanic { .. } => ("command_panic", false),
        poise::FrameworkError::ArgumentParse { .. } => ("argument_parse", false),
        poise::FrameworkError::CommandStructureMismatch { .. } => ("command_structure", false),
        poise::FrameworkError::MissingBotPermissions { .. } => ("bot_permissions", false),
        poise::FrameworkError::MissingUserPermissions { .. }
        | poise::FrameworkError::NotAnOwner { .. } => ("user_permissions", false),
        poise::FrameworkError::GuildOnly { .. }
        | poise::FrameworkError::DmOnly { .. }
        | poise::FrameworkError::NsfwOnly { .. } => ("command_scope", false),
        poise::FrameworkError::SubcommandRequired { .. } => ("subcommand_required", false),
        poise::FrameworkError::CooldownHit { .. } => ("cooldown", false),
        poise::FrameworkError::CommandCheckFailed { .. } => ("command_check", false),
        poise::FrameworkError::Setup { .. } => ("framework_setup", false),
        poise::FrameworkError::EventHandler { .. } => ("event_handler", false),
        poise::FrameworkError::UnknownInteraction { .. } => ("unknown_interaction", false),
        poise::FrameworkError::UnknownCommand { .. } => ("unknown_command", false),
        poise::FrameworkError::DynamicPrefix { .. }
        | poise::FrameworkError::NonCommandMessage { .. } => ("prefix_command", false),
        _ => ("framework", false),
    }
}

fn command_error_class(error: &CommandError) -> &'static str {
    if error
        .downcast_ref::<crate::commands::response::AppliedResponseError>()
        .is_some()
    {
        "response_after_apply"
    } else if error
        .downcast_ref::<crate::commands::PlayInputError>()
        .is_some()
    {
        "play_input"
    } else if error.downcast_ref::<crate::VoicePolicyError>().is_some()
        || error
            .downcast_ref::<crate::voice_facts::CurrentVoiceFactsError>()
            .is_some()
    {
        "voice_policy"
    } else if error.downcast_ref::<crate::ResolveError>().is_some() {
        "media_resolution"
    } else if error
        .downcast_ref::<pepeaudio_player::PlayerError>()
        .is_some()
    {
        "player"
    } else if error.downcast_ref::<crate::RegistryError>().is_some() {
        "player_registry"
    } else if error.downcast_ref::<crate::RestBoundaryError>().is_some() {
        "discord_response"
    } else if error
        .downcast_ref::<pepeaudio_components_v2::ValidationError>()
        .is_some()
    {
        "response_payload"
    } else if error.downcast_ref::<crate::GuildPolicyError>().is_some() {
        "guild_policy"
    } else if error
        .downcast_ref::<pepeaudio_core::SnowflakeParseError>()
        .is_some()
    {
        "discord_state"
    } else {
        "command"
    }
}

#[cfg(test)]
mod tests {
    use super::{command_error_class, media_failure_message, play_input_failure_message};

    #[test]
    fn classifications_do_not_depend_on_error_messages() {
        let voice: crate::CommandError = Box::new(crate::VoicePolicyError::MissingPrivilege);
        let media: crate::CommandError = Box::new(crate::ResolveError::Failed(
            "sensitive source details".into(),
        ));
        let applied = crate::commands::response::applied_response_error(std::io::Error::other(
            "sensitive response details",
        ));
        let play_input: crate::CommandError = Box::new(crate::commands::PlayInputError::Missing);

        assert_eq!(command_error_class(&voice), "voice_policy");
        assert_eq!(command_error_class(&media), "media_resolution");
        assert_eq!(command_error_class(&applied), "response_after_apply");
        assert_eq!(command_error_class(&play_input), "play_input");
    }

    #[test]
    fn play_input_failures_have_specific_safe_public_copy() {
        let missing = play_input_failure_message(&crate::commands::PlayInputError::Missing);
        let conflicting = play_input_failure_message(&crate::commands::PlayInputError::Conflicting);

        assert!(missing.contains("URLまたは音声ファイル"));
        assert!(missing.contains("どちらか一方"));
        assert!(conflicting.contains("同時に指定できません"));
    }

    #[test]
    fn catalog_failures_have_specific_safe_public_copy() {
        let spotify =
            media_failure_message(&crate::ResolveError::SpotifyPlaylistRequiresUserAuthorization)
                .expect("specific copy");
        let spotify_album =
            media_failure_message(&crate::ResolveError::SpotifyAlbumRequiresCredentials)
                .expect("Spotify album copy");
        let apple = media_failure_message(
            &crate::ResolveError::AppleMusicPlaylistRequiresDeveloperCredentials,
        )
        .expect("specific copy");
        let no_match =
            media_failure_message(&crate::ResolveError::NoSearchMatch).expect("specific copy");
        let unavailable = media_failure_message(&crate::ResolveError::UnsupportedStream)
            .expect("unavailable media copy");
        assert!(spotify.contains("Spotifyユーザー認証"));
        assert!(spotify_album.contains("credential overlay"));
        assert!(apple.contains("Apple Developer"));
        assert!(apple.contains("曲とアルバム"));
        assert!(no_match.contains("YouTube"));
        assert!(unavailable.contains("動画が公開中"));
        assert!(media_failure_message(&crate::ResolveError::Failed("secret".into())).is_none());
    }
}
