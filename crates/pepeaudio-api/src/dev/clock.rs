use std::sync::atomic::{AtomicU64, Ordering};

use pepeaudio_core::UnixTimeMillis;

use crate::Clock;

#[derive(Debug)]
pub struct ManualClock {
    millis: AtomicU64,
}

impl ManualClock {
    #[must_use]
    pub const fn new(millis: u64) -> Self {
        Self {
            millis: AtomicU64::new(millis),
        }
    }

    pub fn set(&self, millis: u64) {
        self.millis.store(millis, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> UnixTimeMillis {
        UnixTimeMillis::new(self.millis.load(Ordering::SeqCst))
    }
}
