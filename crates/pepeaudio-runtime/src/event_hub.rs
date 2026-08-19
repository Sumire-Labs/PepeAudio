use std::{
    collections::HashMap,
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use pepeaudio_api::PlayerEvent;
use pepeaudio_core::{GuildId, PlayerSnapshot};
use tokio::sync::broadcast;

pub(crate) struct EventHub {
    capacity: usize,
    guilds: RwLock<HashMap<GuildId, broadcast::Sender<PlayerEvent>>>,
}

impl EventHub {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            guilds: RwLock::new(HashMap::new()),
        }
    }

    pub(crate) fn subscribe(&self, guild_id: GuildId) -> broadcast::Receiver<PlayerEvent> {
        if let Some(sender) = read_unpoisoned(&self.guilds).get(&guild_id) {
            return sender.subscribe();
        }
        let sender = write_unpoisoned(&self.guilds)
            .entry(guild_id)
            .or_insert_with(|| broadcast::channel(self.capacity).0)
            .clone();
        sender.subscribe()
    }

    pub(crate) fn publish(&self, snapshot: PlayerSnapshot) {
        let sender = read_unpoisoned(&self.guilds)
            .get(&snapshot.guild_id)
            .cloned();
        if let Some(sender) = sender {
            let _receivers = sender.send(PlayerEvent { snapshot });
        }
    }
}

fn read_unpoisoned<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn write_unpoisoned<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
