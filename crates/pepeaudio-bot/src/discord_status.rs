use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use poise::serenity_prelude as serenity;
use serenity::gateway::{ActivityData, ShardManager};
use tokio::{sync::watch, task::JoinHandle, time::MissedTickBehavior};

use crate::{BotError, process_memory::ProcessMemory};

pub(crate) const STATUS_UPDATE_INTERVAL: Duration = Duration::from_mins(1);
const MIBIBYTE: u64 = 1024 * 1024;
const GIBIBYTE: u64 = 1024 * MIBIBYTE;

trait MemoryProbe: Send + 'static {
    fn resident_bytes(&mut self) -> Option<u64>;
}

impl MemoryProbe for ProcessMemory {
    fn resident_bytes(&mut self) -> Option<u64> {
        ProcessMemory::resident_bytes(self)
    }
}

#[async_trait]
trait StatusPublisher: Send + Sync + 'static {
    async fn publish(&self, message: &str);
}

struct ShardStatusPublisher {
    manager: Arc<ShardManager>,
}

#[async_trait]
impl StatusPublisher for ShardStatusPublisher {
    async fn publish(&self, message: &str) {
        let messengers: Vec<_> = self
            .manager
            .runners
            .lock()
            .await
            .values()
            .map(|runner| runner.runner_tx.clone())
            .collect();

        for messenger in messengers {
            messenger.set_activity(Some(ActivityData::custom(message)));
        }
    }
}

pub(crate) struct DiscordStatusRuntime {
    shutdown: watch::Sender<bool>,
    task: Option<JoinHandle<()>>,
}

impl DiscordStatusRuntime {
    pub(crate) fn start(manager: Arc<ShardManager>) -> Self {
        Self::start_with(
            Arc::new(ShardStatusPublisher { manager }),
            Box::new(ProcessMemory::new()),
            STATUS_UPDATE_INTERVAL,
        )
    }

    fn start_with(
        publisher: Arc<dyn StatusPublisher>,
        probe: Box<dyn MemoryProbe>,
        interval: Duration,
    ) -> Self {
        let (shutdown, receiver) = watch::channel(false);
        let task = tokio::spawn(run_updates(publisher, probe, interval, receiver));
        Self {
            shutdown,
            task: Some(task),
        }
    }

    pub(crate) async fn wait_for_unexpected_exit(&mut self) -> BotError {
        let Some(task) = self.task.as_mut() else {
            return BotError::DiscordStatusStopped;
        };
        let outcome = task.await;
        self.task.take();
        match outcome {
            Ok(()) => BotError::DiscordStatusStopped,
            Err(error) => BotError::DiscordStatusTask(error),
        }
    }

    pub(crate) async fn shutdown(mut self) -> bool {
        let Some(task) = self.task.take() else {
            return true;
        };
        let signalled = self.shutdown.send(true).is_ok();
        let joined = task.await.is_ok();
        signalled && joined
    }
}

impl Drop for DiscordStatusRuntime {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

pub(crate) fn initial_activity() -> Option<ActivityData> {
    ProcessMemory::new()
        .resident_bytes()
        .map(|bytes| ActivityData::custom(status_message(bytes)))
}

async fn run_updates(
    publisher: Arc<dyn StatusPublisher>,
    mut probe: Box<dyn MemoryProbe>,
    interval: Duration,
    mut shutdown: watch::Receiver<bool>,
) {
    let start = tokio::time::Instant::now() + interval;
    let mut ticks = tokio::time::interval_at(start, interval);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut sample_was_unavailable = false;

    loop {
        tokio::select! {
            biased;
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return;
                }
            }
            _ = ticks.tick() => {
                if let Some(bytes) = probe.resident_bytes() {
                    if sample_was_unavailable {
                        tracing::info!("Bot memory sampling recovered");
                    }
                    sample_was_unavailable = false;
                    publisher.publish(&status_message(bytes)).await;
                } else if !sample_was_unavailable {
                    sample_was_unavailable = true;
                    tracing::warn!("Bot memory sample is temporarily unavailable");
                }
            }
        }
    }
}

fn status_message(bytes: u64) -> String {
    if bytes >= GIBIBYTE {
        let hundredths =
            (u128::from(bytes) * 100 + u128::from(GIBIBYTE / 2)) / u128::from(GIBIBYTE);
        format!("Bot RAM · {}.{:02} GiB", hundredths / 100, hundredths % 100)
    } else {
        let mibibytes = bytes.saturating_add(MIBIBYTE / 2) / MIBIBYTE;
        format!("Bot RAM · {mibibytes} MiB")
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use tokio::time::Instant;

    use super::{
        DiscordStatusRuntime, GIBIBYTE, MIBIBYTE, MemoryProbe, StatusPublisher, status_message,
    };

    struct SequenceProbe {
        samples: VecDeque<Option<u64>>,
    }

    impl MemoryProbe for SequenceProbe {
        fn resident_bytes(&mut self) -> Option<u64> {
            self.samples.pop_front().flatten()
        }
    }

    #[derive(Default)]
    struct RecordingPublisher {
        messages: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl StatusPublisher for RecordingPublisher {
        async fn publish(&self, message: &str) {
            self.messages
                .lock()
                .expect("recording lock")
                .push(message.to_owned());
        }
    }

    #[test]
    fn status_uses_compact_binary_units() {
        assert_eq!(status_message(128 * MIBIBYTE), "Bot RAM · 128 MiB");
        assert_eq!(
            status_message(GIBIBYTE + GIBIBYTE / 2),
            "Bot RAM · 1.50 GiB"
        );
        assert!(status_message(u64::MAX).len() <= 128);
    }

    #[tokio::test(start_paused = true)]
    async fn first_periodic_update_waits_for_the_full_interval() {
        let publisher = Arc::new(RecordingPublisher::default());
        let runtime = DiscordStatusRuntime::start_with(
            publisher.clone(),
            Box::new(SequenceProbe {
                samples: VecDeque::from([Some(64 * MIBIBYTE)]),
            }),
            Duration::from_mins(1),
        );
        tokio::task::yield_now().await;

        tokio::time::advance(Duration::from_secs(59)).await;
        tokio::task::yield_now().await;
        assert!(
            publisher
                .messages
                .lock()
                .expect("recording lock")
                .is_empty()
        );

        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(
            publisher
                .messages
                .lock()
                .expect("recording lock")
                .as_slice(),
            ["Bot RAM · 64 MiB"]
        );
        assert!(runtime.shutdown().await);
    }

    #[tokio::test(start_paused = true)]
    async fn missed_ticks_do_not_burst_presence_updates() {
        let publisher = Arc::new(RecordingPublisher::default());
        let runtime = DiscordStatusRuntime::start_with(
            publisher.clone(),
            Box::new(SequenceProbe {
                samples: VecDeque::from([
                    Some(64 * MIBIBYTE),
                    Some(65 * MIBIBYTE),
                    Some(66 * MIBIBYTE),
                ]),
            }),
            Duration::from_mins(1),
        );
        tokio::task::yield_now().await;
        let started = Instant::now();

        tokio::time::advance(Duration::from_mins(5)).await;
        tokio::task::yield_now().await;

        assert_eq!(Instant::now() - started, Duration::from_mins(5));
        assert_eq!(publisher.messages.lock().expect("recording lock").len(), 1);
        assert!(runtime.shutdown().await);
    }

    #[tokio::test(start_paused = true)]
    async fn unavailable_samples_leave_the_last_discord_status_unchanged() {
        let publisher = Arc::new(RecordingPublisher::default());
        let runtime = DiscordStatusRuntime::start_with(
            publisher.clone(),
            Box::new(SequenceProbe {
                samples: VecDeque::from([None, Some(96 * MIBIBYTE)]),
            }),
            Duration::from_mins(1),
        );
        tokio::task::yield_now().await;

        tokio::time::advance(Duration::from_mins(1)).await;
        tokio::task::yield_now().await;
        assert!(
            publisher
                .messages
                .lock()
                .expect("recording lock")
                .is_empty()
        );

        tokio::time::advance(Duration::from_mins(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(
            publisher
                .messages
                .lock()
                .expect("recording lock")
                .as_slice(),
            ["Bot RAM · 96 MiB"]
        );
        assert!(runtime.shutdown().await);
    }
}
