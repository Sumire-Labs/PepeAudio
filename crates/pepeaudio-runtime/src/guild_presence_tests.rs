use std::{collections::HashMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use pepeaudio_core::GuildId;
use pepeaudio_storage::{BotPresenceStore, StorageResult};
use tokio::sync::{Mutex, mpsc};

use super::{GuildPresenceHandle, GuildPresenceRuntime};
use crate::RuntimeError;

#[test]
fn rejects_nonexpiring_or_overlapping_heartbeat_configuration() {
    struct NeverStore;
    #[async_trait]
    impl BotPresenceStore for NeverStore {
        async fn refresh_bot_presence(
            &self,
            _: GuildId,
            _: &str,
            _: Duration,
        ) -> StorageResult<()> {
            unreachable!()
        }

        async fn clear_bot_presence(&self, _: GuildId, _: &str) -> StorageResult<bool> {
            unreachable!()
        }

        async fn is_bot_present(&self, _: GuildId) -> StorageResult<bool> {
            unreachable!()
        }
    }
    assert!(
        GuildPresenceRuntime::start(
            NeverStore,
            "bot-0".into(),
            Duration::from_secs(30),
            Duration::from_secs(30)
        )
        .is_err()
    );
}

#[derive(Clone, Default)]
struct FakeStore {
    owners: Arc<Mutex<HashMap<GuildId, String>>>,
    refreshes: Arc<Mutex<usize>>,
    refresh_failures: Arc<Mutex<usize>>,
}

#[async_trait]
impl BotPresenceStore for FakeStore {
    async fn refresh_bot_presence(
        &self,
        guild_id: GuildId,
        instance_id: &str,
        _: Duration,
    ) -> StorageResult<()> {
        let mut failures = self.refresh_failures.lock().await;
        if *failures > 0 {
            *failures -= 1;
            return Err(pepeaudio_storage::StorageError::CapacityExceeded {
                resource: "test presence store",
            });
        }
        drop(failures);
        self.owners
            .lock()
            .await
            .insert(guild_id, instance_id.to_owned());
        *self.refreshes.lock().await += 1;
        Ok(())
    }

    async fn clear_bot_presence(
        &self,
        guild_id: GuildId,
        instance_id: &str,
    ) -> StorageResult<bool> {
        let mut owners = self.owners.lock().await;
        let owned = owners
            .get(&guild_id)
            .is_some_and(|owner| owner == instance_id);
        if owned {
            owners.remove(&guild_id);
        }
        Ok(owned)
    }

    async fn is_bot_present(&self, guild_id: GuildId) -> StorageResult<bool> {
        Ok(self.owners.lock().await.contains_key(&guild_id))
    }
}

#[tokio::test(start_paused = true)]
async fn advertises_heartbeats_and_clears_owned_presence() {
    let store = FakeStore::default();
    let runtime = GuildPresenceRuntime::start(
        store.clone(),
        "bot-0".into(),
        Duration::from_secs(30),
        Duration::from_secs(10),
    )
    .expect("presence runtime");
    let handle = runtime.handle();
    let guild_id = GuildId::new(42).expect("guild");
    handle.present(guild_id).await.expect("present");
    assert!(store.is_bot_present(guild_id).await.expect("lookup"));

    tokio::time::advance(Duration::from_secs(10)).await;
    tokio::task::yield_now().await;
    assert!(*store.refreshes.lock().await >= 2);

    handle.absent(guild_id).await.expect("absent");
    assert!(!store.is_bot_present(guild_id).await.expect("lookup"));
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test(start_paused = true)]
async fn heartbeat_repairs_a_failed_initial_presence_write() {
    let store = FakeStore::default();
    let runtime = GuildPresenceRuntime::start(
        store.clone(),
        "bot-0".into(),
        Duration::from_secs(30),
        Duration::from_secs(10),
    )
    .expect("presence runtime");
    let guild_id = GuildId::new(43).expect("guild");
    tokio::task::yield_now().await;
    *store.refresh_failures.lock().await = 1;

    assert!(runtime.handle().present(guild_id).await.is_err());
    assert!(!store.is_bot_present(guild_id).await.expect("lookup"));
    tokio::time::advance(Duration::from_secs(10)).await;
    tokio::task::yield_now().await;
    assert!(store.is_bot_present(guild_id).await.expect("lookup"));

    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn clean_presence_return_is_an_unexpected_exit() {
    let (sender, _receiver) = mpsc::channel(1);
    let mut runtime = GuildPresenceRuntime {
        handle: GuildPresenceHandle { sender },
        task: Some(tokio::spawn(async {})),
    };

    assert!(matches!(
        runtime.wait_for_unexpected_exit().await,
        RuntimeError::RequiredTaskStopped {
            task: "guild presence"
        }
    ));
}

#[derive(Clone, Default)]
struct SlowHeartbeatStore {
    calls: Arc<Mutex<HashMap<GuildId, usize>>>,
}

#[async_trait]
impl BotPresenceStore for SlowHeartbeatStore {
    async fn refresh_bot_presence(
        &self,
        guild_id: GuildId,
        _: &str,
        _: Duration,
    ) -> StorageResult<()> {
        let mut calls = self.calls.lock().await;
        let call = calls.entry(guild_id).or_default();
        *call += 1;
        let is_heartbeat = *call > 1;
        drop(calls);
        if is_heartbeat {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        Ok(())
    }

    async fn clear_bot_presence(&self, _: GuildId, _: &str) -> StorageResult<bool> {
        Ok(true)
    }

    async fn is_bot_present(&self, _: GuildId) -> StorageResult<bool> {
        Ok(true)
    }
}

#[tokio::test(start_paused = true)]
async fn slow_heartbeats_do_not_block_gateway_presence_commands() {
    let store = SlowHeartbeatStore::default();
    let runtime = GuildPresenceRuntime::start(
        store,
        "bot-0".into(),
        Duration::from_secs(30),
        Duration::from_secs(10),
    )
    .expect("presence runtime");
    let handle = runtime.handle();
    tokio::task::yield_now().await;

    for id in 1..=41 {
        handle
            .present(GuildId::new(id).expect("guild"))
            .await
            .expect("initial presence");
    }
    tokio::time::advance(Duration::from_secs(10)).await;
    tokio::task::yield_now().await;

    tokio::time::timeout(
        Duration::from_secs(1),
        handle.present(GuildId::new(99).expect("new guild")),
    )
    .await
    .expect("heartbeat sweep must not block actor commands")
    .expect("new guild presence");

    runtime.shutdown().await.expect("shutdown");
}

struct HungStore;

#[async_trait]
impl BotPresenceStore for HungStore {
    async fn refresh_bot_presence(&self, _: GuildId, _: &str, _: Duration) -> StorageResult<()> {
        std::future::pending().await
    }

    async fn clear_bot_presence(&self, _: GuildId, _: &str) -> StorageResult<bool> {
        Ok(true)
    }

    async fn is_bot_present(&self, _: GuildId) -> StorageResult<bool> {
        Ok(false)
    }
}

#[tokio::test(start_paused = true)]
async fn an_unresponsive_store_cannot_hold_the_presence_mailbox_forever() {
    let runtime = GuildPresenceRuntime::start(
        HungStore,
        "bot-0".into(),
        Duration::from_secs(30),
        Duration::from_secs(10),
    )
    .expect("presence runtime");
    let handle = runtime.handle();
    let request =
        tokio::spawn(async move { handle.present(GuildId::new(44).expect("guild")).await });

    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(2)).await;
    tokio::task::yield_now().await;

    assert!(matches!(
        request.await.expect("request task"),
        Err(RuntimeError::PresenceTimedOut {
            operation: "present"
        })
    ));
    runtime.shutdown().await.expect("shutdown");
}
