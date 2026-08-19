use std::{sync::Arc, time::Duration};

use pepeaudio_core::GuildId;
use pepeaudio_storage::{GuildSettings, GuildSettingsRepository};
use tokio::{
    sync::watch,
    time::{Instant, sleep_until, timeout},
};

use crate::settings_model::{PersistentPlayerSettings, SettingsWorkerState};

#[derive(Clone, Copy)]
pub(crate) struct SettingsWorkerConfig {
    write_timeout: Duration,
    initial_retry: Duration,
    maximum_retry: Duration,
}

impl Default for SettingsWorkerConfig {
    fn default() -> Self {
        Self {
            write_timeout: Duration::from_secs(10),
            initial_retry: Duration::from_millis(100),
            maximum_retry: Duration::from_secs(30),
        }
    }
}

pub(crate) async fn run_settings_worker<R>(
    guild_id: GuildId,
    repository: Arc<R>,
    seed: GuildSettings,
    mailbox: watch::Sender<SettingsWorkerState>,
    mut receiver: watch::Receiver<SettingsWorkerState>,
    mut shutdown: watch::Receiver<bool>,
    config: SettingsWorkerConfig,
) -> Result<(), SettingsWorkerError>
where
    R: GuildSettingsRepository,
{
    let mut current = seed;
    let mut retry = config.initial_retry;
    loop {
        let pending = receiver.borrow_and_update().pending.clone();
        let Some(update) = pending else {
            tokio::select! {
                changed = receiver.changed() => {
                    if changed.is_err() {
                        return Ok(());
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        return Ok(());
                    }
                }
            }
            continue;
        };

        if let Ok(Ok(updated)) = timeout(
            config.write_timeout,
            persist_latest(repository.as_ref(), guild_id, &current, &update.settings),
        )
        .await
        {
            current = updated.clone();
            retry = config.initial_retry;
            mailbox.send_modify(|state| {
                state.durable = updated;
                if state.pending.as_ref().map(|item| item.actor_revision)
                    == Some(update.actor_revision)
                {
                    state.pending = None;
                }
            });
        } else {
            if *shutdown.borrow() {
                return Err(SettingsWorkerError::FinalWriteFailed);
            }
            let deadline = Instant::now() + retry;
            tokio::select! {
                () = sleep_until(deadline) => {}
                changed = receiver.changed() => {
                    if changed.is_err() {
                        return Ok(());
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        let latest = receiver.borrow().pending.clone();
                        if let Some(latest) = latest {
                            return final_flush(
                                repository.as_ref(),
                                guild_id,
                                &current,
                                &latest.settings,
                                config.write_timeout,
                            )
                            .await;
                        }
                        return Ok(());
                    }
                }
            }
            retry = retry.saturating_mul(2).min(config.maximum_retry);
        }
    }
}

async fn final_flush<R>(
    repository: &R,
    guild_id: GuildId,
    current: &GuildSettings,
    desired: &PersistentPlayerSettings,
    write_timeout: Duration,
) -> Result<(), SettingsWorkerError>
where
    R: GuildSettingsRepository,
{
    timeout(
        write_timeout,
        persist_latest(repository, guild_id, current, desired),
    )
    .await
    .map_err(|_| SettingsWorkerError::FinalWriteFailed)?
    .map(|_| ())
    .map_err(|_| SettingsWorkerError::FinalWriteFailed)
}

async fn persist_latest<R>(
    repository: &R,
    guild_id: GuildId,
    seed: &GuildSettings,
    desired: &PersistentPlayerSettings,
) -> Result<GuildSettings, SettingsWorkerError>
where
    R: GuildSettingsRepository,
{
    let mut current = if seed.guild_id == guild_id {
        seed.clone()
    } else {
        repository
            .get_guild_settings(guild_id)
            .await
            .map_err(|_| SettingsWorkerError::Repository)?
            .ok_or(SettingsWorkerError::Repository)?
    };
    for _ in 0..8 {
        if matches_desired(&current, desired) {
            return Ok(current);
        }
        let expected = current.revision;
        let mut candidate = current.clone();
        candidate.volume = desired.volume;
        candidate.default_hrir_preset_id = Some(desired.hrir_preset.clone());
        candidate.spatial_audio_enabled = desired.spatial_audio_enabled;
        if let Some(updated) = repository
            .update_guild_settings(&candidate, expected)
            .await
            .map_err(|_| SettingsWorkerError::Repository)?
        {
            return Ok(updated);
        }
        current = repository
            .get_guild_settings(guild_id)
            .await
            .map_err(|_| SettingsWorkerError::Repository)?
            .ok_or(SettingsWorkerError::Repository)?;
    }
    Err(SettingsWorkerError::Conflict)
}

fn matches_desired(current: &GuildSettings, desired: &PersistentPlayerSettings) -> bool {
    current.volume == desired.volume
        && current.default_hrir_preset_id.as_ref() == Some(&desired.hrir_preset)
        && current.spatial_audio_enabled == desired.spatial_audio_enabled
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsWorkerError {
    Repository,
    Conflict,
    FinalWriteFailed,
}
