// SPDX-License-Identifier: Apache-2.0
using Discord.WebSocket;
using Microsoft.Extensions.Logging;
using PepeAudio.Audio;
using PepeAudio.Core;
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Sharding;

namespace PepeAudio.Host.Lifecycle;

// On shard READY, resumes playback for that shard's guilds from durable checkpoints,
// but only where the voice channel still has a human listener (else the checkpoint
// is discarded so the bot never rejoins an empty channel).
public sealed class PlaybackRestorer
{
    private readonly ICheckpointStore _checkpoints;
    private readonly IPlayerManager _players;
    private readonly IShardTopology _topology;
    private readonly DiscordShardedClient _client;
    private readonly ILogger<PlaybackRestorer> _log;

    public PlaybackRestorer(ICheckpointStore checkpoints, IPlayerManager players, IShardTopology topology,
        DiscordShardedClient client, ILogger<PlaybackRestorer> log)
    {
        _checkpoints = checkpoints;
        _players = players;
        _topology = topology;
        _client = client;
        _log = log;
    }

    public async Task RestoreShardAsync(int shardId)
    {
        foreach (var cp in await _checkpoints.LoadAllAsync(CancellationToken.None))
        {
            if (GuildShardMap.ShardIdFor(cp.GuildId, _topology.TotalShards) != shardId || !_topology.Owns(cp.GuildId))
                continue;

            // A shard reconnect must not re-restore a guild that is already playing (rewind).
            if (_players.TryGet(cp.GuildId, out var existing) && existing is not null)
                continue;

            var channel = _client.GetGuild(cp.GuildId)?.GetVoiceChannel(cp.VoiceChannelId);
            var listeners = channel?.ConnectedUsers.Count(u => !u.IsBot) ?? 0;
            if (channel is null || listeners == 0)
            {
                await _checkpoints.DeleteAsync(cp.GuildId, CancellationToken.None);
                continue;
            }

            try
            {
                await _players.GetOrCreate(cp.GuildId).RestoreAsync(cp, CancellationToken.None);
                _log.LogInformation("Resumed playback in guild {Guild} ({Listeners} listener(s))", cp.GuildId, listeners);
            }
            catch (Exception ex)
            {
                _log.LogWarning(ex, "Restore failed for guild {Guild}", cp.GuildId);
                await _checkpoints.DeleteAsync(cp.GuildId, CancellationToken.None);
            }
        }
    }
}
