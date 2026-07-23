// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Audio.Effects;
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Models;

namespace PepeAudio.Audio;

public interface IGuildPlayer
{
    ulong GuildId { get; }
    ulong VoiceChannelId { get; }
    EffectSettings CurrentSettings { get; }
    void SetVoiceChannel(ulong channelId);
    void ApplySettings(GuildSettings settings);
    Task EnsureConnectedAsync(ulong voiceChannelId, CancellationToken ct);
    void Enqueue(PlayableRef track);
    void Apply(ControlEnvelope envelope);
    Task RestoreAsync(PlayerCheckpoint checkpoint, CancellationToken ct);
    Task DrainAsync();
    Task StopAsync();
    PlayerState Snapshot();
}

public interface IPlayerManager
{
    IGuildPlayer GetOrCreate(ulong guildId);
    bool TryGet(ulong guildId, out IGuildPlayer? player);
    Task RemoveAsync(ulong guildId);
    Task DrainAllAsync();
    int ActiveCount { get; }
    IReadOnlyCollection<ulong> ActiveGuildIds { get; }
}
