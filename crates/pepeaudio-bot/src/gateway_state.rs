use std::{collections::HashSet, sync::Arc};

use poise::serenity_prelude as serenity;

use crate::{BotData, GuildLifecycleHandle};

pub(crate) async fn update_gateway_state(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    data: &BotData,
) {
    match event {
        serenity::FullEvent::Ready { data_about_bot } => {
            if let Some(lifecycle) = &data.guild_lifecycle {
                reconcile_ready_shard(data, lifecycle, ctx.shard_id.get(), &data_about_bot.guilds)
                    .await;
            }
        }
        serenity::FullEvent::CacheReady { guilds } => {
            // CacheReady is process-wide in Serenity. It is useful for voice
            // recovery, but is not a shard ownership boundary.
            for guild_id in guilds {
                reconcile_cached_bot_voice(&ctx.cache, data, *guild_id).await;
            }
        }
        serenity::FullEvent::GuildCreate { guild, .. } => {
            advertise_guild(data, ctx.shard_id.get(), guild.id).await;
            reconcile_cached_bot_voice(&ctx.cache, data, guild.id).await;
        }
        serenity::FullEvent::GuildDelete { incomplete, .. } if !incomplete.unavailable => {
            remove_guild(data, ctx.shard_id.get(), incomplete.id).await;
        }
        serenity::FullEvent::VoiceStateUpdate { new, .. }
            if new.user_id == ctx.cache.current_user().id =>
        {
            reconcile_bot_voice_state(data, new).await;
        }
        _ => {}
    }
}

async fn reconcile_ready_shard(
    data: &BotData,
    lifecycle: &GuildLifecycleHandle,
    shard_id: u32,
    discord_guilds: &[serenity::UnavailableGuild],
) {
    let desired: HashSet<_> = discord_guilds
        .iter()
        .filter_map(|guild| pepeaudio_core::GuildId::new(guild.id.get()).ok())
        .collect();
    match lifecycle.reconcile_shard(shard_id, desired).await {
        Ok(outcome) => {
            if outcome.update.is_err() {
                tracing::warn!(shard_id, "guild lifecycle reconciliation is pending");
            }
            for guild_id in outcome.removed {
                remove_player(data, guild_id).await;
            }
        }
        Err(_) => {
            tracing::warn!(shard_id, "guild lifecycle actor is unavailable");
        }
    }
}

async fn advertise_guild(data: &BotData, shard_id: u32, discord_guild_id: serenity::GuildId) {
    let (Some(lifecycle), Ok(guild_id)) = (
        &data.guild_lifecycle,
        pepeaudio_core::GuildId::new(discord_guild_id.get()),
    ) else {
        return;
    };
    advertise_core_guild(lifecycle, shard_id, guild_id).await;
}

async fn advertise_core_guild(
    lifecycle: &GuildLifecycleHandle,
    shard_id: u32,
    guild_id: pepeaudio_core::GuildId,
) {
    if lifecycle
        .present_on_shard(shard_id, guild_id)
        .await
        .is_err()
    {
        tracing::warn!(guild_id = guild_id.get(), "guild lifecycle update failed");
    }
}

async fn remove_guild(data: &BotData, shard_id: u32, discord_guild_id: serenity::GuildId) {
    let Ok(guild_id) = pepeaudio_core::GuildId::new(discord_guild_id.get()) else {
        return;
    };
    if let Some(lifecycle) = &data.guild_lifecycle {
        match lifecycle.remove_from_shard(shard_id, guild_id).await {
            Ok(outcome) => {
                if outcome.update.is_err() {
                    tracing::warn!(
                        guild_id = guild_id.get(),
                        "guild presence cleanup remains pending"
                    );
                }
                if outcome.no_longer_owned {
                    remove_player(data, guild_id).await;
                }
            }
            Err(_) => {
                tracing::warn!(
                    guild_id = guild_id.get(),
                    "guild lifecycle actor is unavailable"
                );
            }
        }
    } else {
        remove_player(data, guild_id).await;
    }
}

async fn remove_player(data: &BotData, guild_id: pepeaudio_core::GuildId) {
    if let Err(error) = data.players.remove_and_shutdown(guild_id).await {
        warn_player_cleanup(guild_id, &error);
    }
}

fn warn_player_cleanup(guild_id: pepeaudio_core::GuildId, error: &crate::RegistryError) {
    tracing::warn!(
        guild_id = guild_id.get(),
        error = %error,
        "removed guild player cleanup failed"
    );
}

async fn reconcile_cached_bot_voice(
    cache: &Arc<serenity::Cache>,
    data: &BotData,
    discord_guild_id: serenity::GuildId,
) {
    let channel_id = cache.guild(discord_guild_id).and_then(|guild| {
        guild
            .voice_states
            .get(&cache.current_user().id)
            .and_then(|voice| voice.channel_id)
    });
    let Ok(guild_id) = pepeaudio_core::GuildId::new(discord_guild_id.get()) else {
        return;
    };
    let Ok(channel_id) = channel_id
        .map(|channel| pepeaudio_core::ChannelId::new(channel.get()))
        .transpose()
    else {
        tracing::warn!(
            guild_id = guild_id.get(),
            "cached bot voice channel was invalid"
        );
        return;
    };
    reconcile_bot_voice_channel(data, guild_id, channel_id).await;
}

async fn reconcile_bot_voice_state(data: &BotData, voice: &serenity::VoiceState) {
    let Some(discord_guild_id) = voice.guild_id else {
        tracing::warn!("bot voice-state update did not identify a guild");
        return;
    };
    let Ok(guild_id) = pepeaudio_core::GuildId::new(discord_guild_id.get()) else {
        tracing::warn!("bot voice-state update contained an invalid guild ID");
        return;
    };
    let Ok(channel_id) = voice
        .channel_id
        .map(|channel| pepeaudio_core::ChannelId::new(channel.get()))
        .transpose()
    else {
        tracing::warn!(
            guild_id = guild_id.get(),
            "bot voice-state update contained an invalid channel ID"
        );
        return;
    };
    reconcile_bot_voice_channel(data, guild_id, channel_id).await;
}

async fn reconcile_bot_voice_channel(
    data: &BotData,
    guild_id: pepeaudio_core::GuildId,
    channel_id: Option<pepeaudio_core::ChannelId>,
) {
    let Some(player) = data.players.get(guild_id).await else {
        return;
    };
    if let Err(error) = player.reconcile_voice_channel(channel_id).await {
        tracing::warn!(
            guild_id = guild_id.get(),
            error = %error,
            "bot voice-state reconciliation failed"
        );
    }
}
