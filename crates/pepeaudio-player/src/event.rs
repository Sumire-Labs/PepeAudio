use std::time::Duration;

use pepeaudio_core::{PlayerSnapshot, StateRevision};
use uuid::Uuid;

use crate::SideEffect;

/// Only [`Self::Natural`] is eligible for repeat processing. Failure reasons
/// are kept distinct so adapters can preserve useful diagnostics without
/// allowing a broken source to enter an unbounded repeat loop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackEndReason {
    Natural,
    WorkerFailed,
    SongbirdEnded,
    SongbirdError,
}

impl PlaybackEndReason {
    pub(crate) const fn is_natural(self) -> bool {
        matches!(self, Self::Natural)
    }
}

/// Monotonic identity assigned to each started or sought playback instance.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlaybackGeneration(u64);

impl PlaybackGeneration {
    pub(crate) fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PlaybackIdentity {
    track_id: Uuid,
    generation: PlaybackGeneration,
}

impl PlaybackIdentity {
    #[must_use]
    pub const fn new(track_id: Uuid, generation: PlaybackGeneration) -> Self {
        Self {
            track_id,
            generation,
        }
    }

    #[must_use]
    pub const fn track_id(self) -> Uuid {
        self.track_id
    }

    #[must_use]
    pub const fn generation(self) -> PlaybackGeneration {
        self.generation
    }
}

/// Monotonic identity assigned whenever the idle timer is armed or invalidated.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct IdleGeneration(u64);

impl IdleGeneration {
    pub(crate) fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlayerEvent {
    StateChanged(Box<PlayerSnapshot>),
    IdleTimerArmed {
        generation: IdleGeneration,
        timeout: Duration,
    },
    IdleTimerCancelled {
        generation: IdleGeneration,
    },
    StaleIdleTimerIgnored {
        expired: IdleGeneration,
        current: IdleGeneration,
    },
    IdleDisconnected {
        generation: IdleGeneration,
    },
    /// Publication happens after actor state has already been committed.
    SnapshotPublicationFailed {
        revision: StateRevision,
        message: String,
    },
    BackgroundSideEffectFailed {
        operation: SideEffect,
        message: String,
    },
    Shutdown(ShutdownReport),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ShutdownReport {
    /// Adapter error when voice cleanup could not be confirmed.
    ///
    /// When present, the actor keeps its state and remains available so the
    /// caller can retry cleanup without losing the Songbird connection handle.
    pub disconnect_error: Option<String>,
    /// Revision after cleanup, or the unchanged revision after a failed attempt.
    pub final_revision: StateRevision,
}
