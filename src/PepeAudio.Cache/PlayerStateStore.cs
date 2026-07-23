// SPDX-License-Identifier: Apache-2.0
using System.Text.Json;
using Microsoft.Extensions.Logging;
using PepeAudio.Core.Contracts;

namespace PepeAudio.Cache;

public interface IPlayerStateStore
{
    Task SetAsync(PlayerState state);
    Task<PlayerState?> GetAsync(ulong guildId);
}

// Best-effort mirror of the canonical PlayerState for fast /now and WebGUI sync.
public sealed class PlayerStateStore : IPlayerStateStore
{
    private readonly IValkeyConnection _valkey;
    private readonly ILogger<PlayerStateStore> _log;

    public PlayerStateStore(IValkeyConnection valkey, ILogger<PlayerStateStore> log)
    {
        _valkey = valkey;
        _log = log;
    }

    public async Task SetAsync(PlayerState state)
    {
        var db = _valkey.TryGetDatabase();
        if (db is null) return;
        try
        {
            var json = JsonSerializer.Serialize(state);
            await db.StringSetAsync(ValkeyKeys.Player(state.GuildId), json, TimeSpan.FromHours(6));
        }
        catch (Exception ex)
        {
            _log.LogDebug(ex, "PlayerState mirror write skipped for {Guild}", state.GuildId);
        }
    }

    public async Task<PlayerState?> GetAsync(ulong guildId)
    {
        var db = _valkey.TryGetDatabase();
        if (db is null) return null;
        try
        {
            var json = await db.StringGetAsync(ValkeyKeys.Player(guildId));
            return json.HasValue ? JsonSerializer.Deserialize<PlayerState>((string)json!) : null;
        }
        catch (Exception ex)
        {
            _log.LogDebug(ex, "PlayerState mirror read skipped for {Guild}", guildId);
            return null;
        }
    }
}
