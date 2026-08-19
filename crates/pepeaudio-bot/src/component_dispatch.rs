use std::time::{SystemTime, UNIX_EPOCH};

use pepeaudio_core::UnixTimeMillis;
use poise::serenity_prelude as serenity;
use serenity::all::{ComponentInteraction, ComponentInteractionDataKind, Interaction};

use crate::{
    BotData, CommandError, ComponentAction, ControlPolicy, InteractionInput, VoiceContext,
    authorize_guild_control, authorize_voice_control, build_ephemeral_status_panel,
    build_now_panel, gateway_state::update_gateway_state, map_interaction,
    voice_facts::current_voice_context,
};

pub(crate) fn event_handler<'a>(
    ctx: &'a serenity::Context,
    event: &'a serenity::FullEvent,
    _framework: poise::FrameworkContext<'a, BotData, CommandError>,
    data: &'a BotData,
) -> poise::BoxFuture<'a, Result<(), CommandError>> {
    Box::pin(async move {
        update_gateway_state(ctx, event, data).await;
        let serenity::FullEvent::InteractionCreate {
            interaction: Interaction::Component(component),
        } = event
        else {
            return Ok(());
        };
        if !component.data.custom_id.starts_with("pa1.") {
            return Ok(());
        }

        if component.defer_ephemeral(&ctx.http).await.is_err() {
            log_component_failure(component, "defer_response", false);
            return Err("the component acknowledgement could not be delivered".into());
        }
        let (message, mutation_was_applied) = match apply_component(ctx, data, component).await {
            Ok(ComponentApplyOutcome::Applied) => {
                (ComponentApplyOutcome::Applied.public_message(), true)
            }
            Ok(outcome @ ComponentApplyOutcome::PanelRefreshFailed) => {
                log_component_failure(component, "panel_refresh", true);
                (outcome.public_message(), true)
            }
            Err(error) => {
                log_component_failure(component, error.class(), false);
                (error.public_message(), false)
            }
        };
        if respond_status(data, component, message).await.is_err() {
            log_component_failure(component, "status_response", mutation_was_applied);
            return Err("the component status response could not be delivered".into());
        }
        Ok(())
    })
}

async fn apply_component(
    ctx: &serenity::Context,
    data: &BotData,
    component: &ComponentInteraction,
) -> Result<ComponentApplyOutcome, ComponentDispatchError> {
    let decoded = data
        .component_ids
        .decode(&component.data.custom_id)
        .map_err(|_| ComponentDispatchError::InvalidComponent)?;
    let guild_id = component
        .guild_id
        .ok_or(ComponentDispatchError::GuildOnly)?;
    if guild_id.get() != decoded.guild_id.get() {
        return Err(ComponentDispatchError::InvalidComponent);
    }

    let player = data
        .players
        .get(decoded.guild_id)
        .await
        .ok_or(ComponentDispatchError::NoPlayer)?;
    let snapshot = player
        .snapshot()
        .await
        .map_err(|_| ComponentDispatchError::PlayerUnavailable)?;
    let (voice, guild_policy) = voice_context(ctx, data, component).await?;
    if snapshot.voice_channel_id != voice.actor_voice_channel_id {
        return Err(ComponentDispatchError::VoicePolicy);
    }
    if decoded.action == ComponentAction::Stop {
        authorize_voice_control(voice, ControlPolicy::PrivilegedSameVoiceChannel)
            .map_err(|_| ComponentDispatchError::VoicePolicy)?;
    } else {
        authorize_guild_control(voice, guild_policy)
            .map_err(|_| ComponentDispatchError::VoicePolicy)?;
    }
    let envelope = map_interaction(
        decoded,
        interaction_input(&component.data.kind)?,
        voice.actor_user_id,
        &snapshot,
        command_deadline(),
    )
    .map_err(|_| ComponentDispatchError::StaleOrInvalid)?;
    let updated = player
        .apply(envelope)
        .await
        .map_err(|_| ComponentDispatchError::StaleOrInvalid)?;
    let Ok(panel) = build_now_panel(&updated, &data.component_ids, &data.hrir_options) else {
        return Ok(ComponentApplyOutcome::PanelRefreshFailed);
    };
    if data
        .components
        .edit_message(component.channel_id, component.message.id, &panel)
        .await
        .is_err()
    {
        Ok(ComponentApplyOutcome::PanelRefreshFailed)
    } else {
        Ok(ComponentApplyOutcome::Applied)
    }
}

async fn voice_context(
    ctx: &serenity::Context,
    data: &BotData,
    component: &ComponentInteraction,
) -> Result<(VoiceContext, crate::GuildControlPolicy), ComponentDispatchError> {
    let guild_id = component
        .guild_id
        .ok_or(ComponentDispatchError::GuildOnly)?;
    let core_guild_id = pepeaudio_core::GuildId::new(guild_id.get())
        .map_err(|_| ComponentDispatchError::InvalidComponent)?;
    let policy = data
        .guild_policy
        .policy(core_guild_id)
        .await
        .map_err(|_| ComponentDispatchError::PlayerUnavailable)?;

    let voice = current_voice_context(&ctx.cache, guild_id, component.user.id, policy)
        .map_err(|_| ComponentDispatchError::VoicePolicy)?;
    Ok((voice, policy))
}

fn interaction_input(
    kind: &ComponentInteractionDataKind,
) -> Result<InteractionInput, ComponentDispatchError> {
    match kind {
        ComponentInteractionDataKind::Button => Ok(InteractionInput::Button),
        ComponentInteractionDataKind::StringSelect { values } if values.len() == 1 => {
            Ok(InteractionInput::Select(values[0].clone()))
        }
        _ => Err(ComponentDispatchError::InvalidComponent),
    }
}

fn command_deadline() -> UnixTimeMillis {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    UnixTimeMillis::new(
        u64::try_from(now)
            .unwrap_or(u64::MAX)
            .saturating_add(30_000),
    )
}

async fn respond_status(
    data: &BotData,
    component: &ComponentInteraction,
    text: &str,
) -> Result<(), CommandError> {
    let panel = build_ephemeral_status_panel(text)?;
    data.components
        .edit_original_response(component.application_id, &component.token, &panel)
        .await?;
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum ComponentDispatchError {
    InvalidComponent,
    GuildOnly,
    VoicePolicy,
    NoPlayer,
    StaleOrInvalid,
    PlayerUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ComponentApplyOutcome {
    Applied,
    PanelRefreshFailed,
}

impl ComponentApplyOutcome {
    const fn public_message(self) -> &'static str {
        match self {
            Self::Applied => "操作を反映しました。",
            Self::PanelRefreshFailed => {
                "操作は反映されましたが、パネルを更新できませんでした。`/now`を開き直してください。"
            }
        }
    }
}

impl ComponentDispatchError {
    const fn class(self) -> &'static str {
        match self {
            Self::InvalidComponent => "invalid_component",
            Self::GuildOnly => "guild_only",
            Self::VoicePolicy => "voice_policy",
            Self::NoPlayer => "no_player",
            Self::StaleOrInvalid => "stale_component",
            Self::PlayerUnavailable => "player_unavailable",
        }
    }

    const fn public_message(self) -> &'static str {
        match self {
            Self::InvalidComponent => {
                "この操作は検証できませんでした。`/now`を開き直してください。"
            }
            Self::GuildOnly => "この操作はDiscordサーバー内でのみ利用できます。",
            Self::VoicePolicy => {
                "Botと同じボイスチャンネルに参加し、必要な権限を確認してください。"
            }
            Self::NoPlayer => "このサーバーには有効なプレイヤーがありません。",
            Self::StaleOrInvalid => "プレイヤーの状態が変わりました。`/now`を開き直してください。",
            Self::PlayerUnavailable => {
                "現在この操作を完了できません。しばらくして再試行してください。"
            }
        }
    }
}

fn log_component_failure(
    component: &ComponentInteraction,
    error_class: &'static str,
    mutation_was_applied: bool,
) {
    tracing::warn!(
        guild_id = ?component.guild_id.map(serenity::GuildId::get),
        interaction_id = component.id.get(),
        mutation_was_applied,
        error_class,
        "Discord player component operation failed"
    );
}

#[cfg(test)]
mod tests {
    use super::{ComponentApplyOutcome, ComponentDispatchError};

    #[test]
    fn panel_refresh_failure_reports_the_applied_mutation() {
        let message = ComponentApplyOutcome::PanelRefreshFailed.public_message();
        assert!(message.contains("操作は反映されました"));
        assert!(message.contains("/now"));
    }

    #[test]
    fn component_failure_classes_are_stable_and_sanitized() {
        assert_eq!(
            ComponentDispatchError::InvalidComponent.class(),
            "invalid_component"
        );
        assert_eq!(
            ComponentDispatchError::PlayerUnavailable.class(),
            "player_unavailable"
        );
    }
}
