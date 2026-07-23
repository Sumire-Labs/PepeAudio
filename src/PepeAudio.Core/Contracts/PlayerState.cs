// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core.Enums;

namespace PepeAudio.Core.Contracts;

// Canonical player snapshot. Written only by the owning guild's player;
// /now and the WebGUI are read-only consumers.
public sealed record PlayerState(
    ulong GuildId,
    TrackInfo? Current,
    long PositionMs,
    bool IsPlaying,
    int Volume,
    LoopMode Loop,
    bool Shuffle,
    bool Autoplay,
    bool AuraEnabled,
    string PresetName,
    int CrossfadeMs,
    IReadOnlyList<QueueEntry> Queue,
    IReadOnlyList<QueueEntry> History,
    long Epoch,
    DateTimeOffset UpdatedAt)
{
    public static PlayerState Empty(ulong guildId) => new(
        guildId, null, 0, false, 10, LoopMode.Off, false, false,
        true, "Aura", 0, Array.Empty<QueueEntry>(), Array.Empty<QueueEntry>(), 0, DateTimeOffset.UtcNow);
}
