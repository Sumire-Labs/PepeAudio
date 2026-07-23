// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core.Enums;

namespace PepeAudio.Core.Contracts;

// A durable snapshot of a guild's playback, persisted so a restarted host can
// resume where it left off. Stream URLs are never stored (PlayableRef re-resolves).
public sealed record PlayerCheckpoint(
    ulong GuildId,
    ulong VoiceChannelId,
    long PositionMs,
    PlayableRef? Current,
    IReadOnlyList<PlayableRef> Queue,
    LoopMode Loop,
    bool Shuffle);

public interface ICheckpointStore
{
    Task SaveAsync(PlayerCheckpoint checkpoint, CancellationToken ct);
    Task<IReadOnlyList<PlayerCheckpoint>> LoadAllAsync(CancellationToken ct);
    Task DeleteAsync(ulong guildId, CancellationToken ct);
}
