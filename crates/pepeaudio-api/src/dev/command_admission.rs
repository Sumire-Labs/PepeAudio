use std::{collections::HashMap, hash::Hash, time::Duration};

use pepeaudio_core::{GuildId, PlayerCommandRateLimit, UnixTimeMillis, UserId};

use crate::RouteError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WindowCounter {
    accepted: u32,
    resets_at_ms: u64,
}

/// Process-local counterpart of production's Valkey admission script.
///
/// This preserves the HTTP behavior in development. It deliberately cannot
/// coordinate independent development API processes.
#[derive(Default)]
pub(super) struct CommandAdmission {
    per_actor: HashMap<(GuildId, UserId), WindowCounter>,
    per_guild: HashMap<GuildId, WindowCounter>,
}

impl CommandAdmission {
    pub(super) fn admit(
        &mut self,
        guild_id: GuildId,
        actor_user_id: Option<UserId>,
        now: UnixTimeMillis,
    ) -> Result<(), RouteError> {
        let actor_user_id = actor_user_id.ok_or(RouteError::InvalidCommand)?;
        let policy = PlayerCommandRateLimit::STANDARD;
        let now_ms = now.get();
        let window_ms = u64::try_from(policy.window().as_millis()).unwrap_or(60_000);
        let new_reset_at = now_ms
            .saturating_sub(now_ms % window_ms)
            .saturating_add(window_ms);
        let actor_key = (guild_id, actor_user_id);
        let actor = active_counter(&self.per_actor, &actor_key, now_ms, new_reset_at);
        let guild = active_counter(&self.per_guild, &guild_id, now_ms, new_reset_at);
        let actor_blocked = actor.accepted >= policy.per_actor_per_guild();
        let guild_blocked = guild.accepted >= policy.per_guild();

        if actor_blocked || guild_blocked {
            let retry_at = [
                actor_blocked.then_some(actor.resets_at_ms),
                guild_blocked.then_some(guild.resets_at_ms),
            ]
            .into_iter()
            .flatten()
            .max()
            .unwrap_or(new_reset_at);
            return Err(RouteError::RateLimited {
                retry_after: retry_after(retry_at, now_ms),
            });
        }

        self.per_actor
            .retain(|_, counter| counter.resets_at_ms > now_ms);
        self.per_guild
            .retain(|_, counter| counter.resets_at_ms > now_ms);
        self.per_actor.insert(
            actor_key,
            WindowCounter {
                accepted: actor.accepted + 1,
                resets_at_ms: actor.resets_at_ms,
            },
        );
        self.per_guild.insert(
            guild_id,
            WindowCounter {
                accepted: guild.accepted + 1,
                resets_at_ms: guild.resets_at_ms,
            },
        );
        Ok(())
    }
}

fn active_counter<K>(
    counters: &HashMap<K, WindowCounter>,
    key: &K,
    now_ms: u64,
    new_reset_at: u64,
) -> WindowCounter
where
    K: Eq + Hash,
{
    counters
        .get(key)
        .copied()
        .filter(|counter| counter.resets_at_ms > now_ms)
        .unwrap_or(WindowCounter {
            accepted: 0,
            resets_at_ms: new_reset_at,
        })
}

fn retry_after(resets_at_ms: u64, now_ms: u64) -> Duration {
    let remaining_ms = resets_at_ms.saturating_sub(now_ms);
    let seconds = remaining_ms.saturating_add(999) / 1_000;
    Duration::from_secs(seconds.clamp(1, 60))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use pepeaudio_core::{GuildId, PlayerCommandRateLimit, UnixTimeMillis, UserId};

    use super::CommandAdmission;
    use crate::RouteError;

    #[test]
    fn actor_rejection_preserves_counters_and_reports_server_delay() {
        let guild_id = GuildId::new(10).expect("guild ID");
        let actor_user_id = UserId::new(20).expect("actor user ID");
        let mut admission = CommandAdmission::default();
        let policy = PlayerCommandRateLimit::STANDARD;
        assert!(policy.per_actor_per_guild() > 0);
        assert!(policy.per_actor_per_guild() <= policy.per_guild());
        assert_eq!(policy.window(), Duration::from_mins(1));

        for _ in 0..policy.per_actor_per_guild() {
            admission
                .admit(guild_id, Some(actor_user_id), UnixTimeMillis::new(1_000))
                .expect("within actor limit");
        }
        let actor_before = admission.per_actor.clone();
        let guild_before = admission.per_guild.clone();

        assert_eq!(
            admission.admit(guild_id, Some(actor_user_id), UnixTimeMillis::new(1_001)),
            Err(RouteError::RateLimited {
                retry_after: Duration::from_secs(59)
            })
        );
        assert_eq!(admission.per_actor, actor_before);
        assert_eq!(admission.per_guild, guild_before);

        assert_eq!(
            admission.admit(guild_id, Some(actor_user_id), UnixTimeMillis::new(59_999)),
            Err(RouteError::RateLimited {
                retry_after: Duration::from_secs(1)
            })
        );
        admission
            .admit(guild_id, Some(actor_user_id), UnixTimeMillis::new(60_000))
            .expect("expired window resets");
    }

    #[test]
    fn guild_rejection_does_not_create_an_actor_window() {
        let guild_id = GuildId::new(10).expect("guild ID");
        let mut admission = CommandAdmission::default();
        let policy = PlayerCommandRateLimit::STANDARD;

        for actor in 1..=policy.per_guild() {
            admission
                .admit(
                    guild_id,
                    Some(UserId::new(u64::from(actor)).expect("actor user ID")),
                    UnixTimeMillis::new(5_000),
                )
                .expect("within guild limit");
        }
        let guild_limit = usize::try_from(policy.per_guild()).expect("guild limit fits usize");
        assert_eq!(admission.per_actor.len(), guild_limit);
        assert_eq!(admission.per_guild.len(), 1);

        let rejected_actor = UserId::new(999).expect("rejected actor user ID");
        assert!(matches!(
            admission.admit(guild_id, Some(rejected_actor), UnixTimeMillis::new(5_000)),
            Err(RouteError::RateLimited { .. })
        ));
        assert!(
            !admission
                .per_actor
                .contains_key(&(guild_id, rejected_actor))
        );
        assert_eq!(admission.per_actor.len(), guild_limit);
    }
}
