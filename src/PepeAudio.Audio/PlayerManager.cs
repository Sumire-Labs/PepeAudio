// SPDX-License-Identifier: Apache-2.0
using System.Collections.Concurrent;
using Discord.WebSocket;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using PepeAudio.Audio.Effects;
using PepeAudio.Cache;
using PepeAudio.Core.Contracts;

namespace PepeAudio.Audio;

public sealed class PlayerManager : IPlayerManager
{
    private readonly ConcurrentDictionary<ulong, GuildPlayer> _players = new();
    private readonly DiscordShardedClient _client;
    private readonly AudioOptions _opt;
    private readonly EffectChainBuilder _chain;
    private readonly IStreamProvider _streams;
    private readonly IAutoplayProvider _autoplay;
    private readonly IValkeyLock _lock;
    private readonly ICheckpointStore _checkpoints;
    private readonly IPlayerStateStore _store;
    private readonly ILoggerFactory _loggerFactory;

    public PlayerManager(DiscordShardedClient client, IOptions<AudioOptions> opt, EffectChainBuilder chain,
        IStreamProvider streams, IAutoplayProvider autoplay, IValkeyLock voiceLock, ICheckpointStore checkpoints,
        IPlayerStateStore store, ILoggerFactory loggerFactory)
    {
        _client = client;
        _opt = opt.Value;
        _chain = chain;
        _streams = streams;
        _autoplay = autoplay;
        _lock = voiceLock;
        _checkpoints = checkpoints;
        _store = store;
        _loggerFactory = loggerFactory;
    }

    public int ActiveCount => _players.Count;
    public IReadOnlyCollection<ulong> ActiveGuildIds => _players.Keys.ToArray();

    public IGuildPlayer GetOrCreate(ulong guildId)
        => _players.GetOrAdd(guildId, id =>
            new GuildPlayer(id, _client, _opt, _chain, _streams, _autoplay, _lock, _checkpoints, _store,
                _loggerFactory.CreateLogger($"GuildPlayer[{id}]"), RemoveAsync));

    public bool TryGet(ulong guildId, out IGuildPlayer? player)
    {
        var found = _players.TryGetValue(guildId, out var gp);
        player = gp;
        return found;
    }

    public async Task RemoveAsync(ulong guildId)
    {
        if (_players.TryRemove(guildId, out var gp))
            await gp.StopAsync();
    }

    // Drains every active player on shutdown: checkpoint, then voice disconnect + lock release.
    public async Task DrainAllAsync()
    {
        foreach (var id in _players.Keys.ToArray())
            if (_players.TryRemove(id, out var gp))
                await gp.DrainAsync();
    }
}
