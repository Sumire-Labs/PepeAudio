use crate::{PlaybackEndReason, PlaybackIdentity, PlayerError, QueueTrack, ShutdownReport};
use pepeaudio_core::{ChannelId, CommandEnvelope, PlayerSnapshot, StateRevision};
use tokio::sync::oneshot;

pub(crate) type MutationReply = oneshot::Sender<Result<PlayerSnapshot, PlayerError>>;

pub(crate) enum PlayerRequest {
    Connect {
        channel_id: ChannelId,
        expected_revision: StateRevision,
        reply: MutationReply,
    },
    Enqueue {
        track: QueueTrack,
        expected_revision: StateRevision,
        reply: MutationReply,
    },
    EnqueueBatch {
        tracks: Vec<QueueTrack>,
        expected_revision: StateRevision,
        reply: MutationReply,
    },
    Apply {
        envelope: CommandEnvelope,
        reply: MutationReply,
    },
    PlaybackEnded {
        identity: PlaybackIdentity,
        reason: PlaybackEndReason,
        reply: MutationReply,
    },
    ReconcileVoiceChannel {
        channel_id: Option<ChannelId>,
        reply: MutationReply,
    },
    Snapshot {
        reply: oneshot::Sender<PlayerSnapshot>,
    },
    Shutdown {
        reply: oneshot::Sender<ShutdownReport>,
    },
}
