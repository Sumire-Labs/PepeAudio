// SPDX-License-Identifier: Apache-2.0
using Microsoft.Extensions.Logging;
using StackExchange.Redis;

namespace PepeAudio.Cache;

// Tracks which instance owns each shard via a heartbeat TTL.
public interface IShardRegistry
{
    Task RegisterAsync(IReadOnlyCollection<int> shardIds, string instanceId, TimeSpan ttl);
    Task RenewAsync(IReadOnlyCollection<int> shardIds, string instanceId, TimeSpan ttl);
    Task ReleaseAsync(IReadOnlyCollection<int> shardIds, string instanceId);
}

public sealed class ValkeyShardRegistry : IShardRegistry
{
    private readonly IValkeyConnection _valkey;
    private readonly ILogger<ValkeyShardRegistry> _log;

    public ValkeyShardRegistry(IValkeyConnection valkey, ILogger<ValkeyShardRegistry> log)
    {
        _valkey = valkey;
        _log = log;
    }

    public async Task RegisterAsync(IReadOnlyCollection<int> shardIds, string instanceId, TimeSpan ttl)
    {
        var db = _valkey.TryGetDatabase();
        if (db is null) return;
        try
        {
            foreach (var id in shardIds)
            {
                var claimed = await db.StringSetAsync(ValkeyKeys.ShardOwner(id), instanceId, ttl, When.NotExists);
                if (!claimed && await db.StringGetAsync(ValkeyKeys.ShardOwner(id)) != instanceId)
                    _log.LogWarning("Shard {Shard} is already owned by another instance (check TOTALSHARDS/SHARDIDS).", id);
                else if (!claimed)
                    await db.KeyExpireAsync(ValkeyKeys.ShardOwner(id), ttl);
            }
            var setKey = $"{ValkeyKeys.Prefix}instance:{instanceId}:shards";
            await db.SetAddAsync(setKey, shardIds.Select(i => (RedisValue)i).ToArray());
            await db.KeyExpireAsync(setKey, ttl); // TTL so the set is reclaimed if this instance crashes
        }
        catch (Exception ex) { _log.LogDebug(ex, "Shard registration skipped (Valkey unavailable)."); }
    }

    public async Task RenewAsync(IReadOnlyCollection<int> shardIds, string instanceId, TimeSpan ttl)
    {
        var db = _valkey.TryGetDatabase();
        if (db is null) return;
        var ms = (RedisValue)(long)ttl.TotalMilliseconds;
        try
        {
            foreach (var id in shardIds)
            {
                var ok = (long)await db.ScriptEvaluateAsync(ValkeyLua.CompareAndPExpire,
                    new RedisKey[] { ValkeyKeys.ShardOwner(id) }, new RedisValue[] { instanceId, ms });
                if (ok == 0) await db.StringSetAsync(ValkeyKeys.ShardOwner(id), instanceId, ttl, When.NotExists);
            }
            await db.KeyExpireAsync($"{ValkeyKeys.Prefix}instance:{instanceId}:shards", ttl);
        }
        catch (Exception ex) { _log.LogDebug(ex, "Shard heartbeat skipped."); }
    }

    public async Task ReleaseAsync(IReadOnlyCollection<int> shardIds, string instanceId)
    {
        var db = _valkey.TryGetDatabase();
        if (db is null) return;
        try
        {
            foreach (var id in shardIds)
                await db.ScriptEvaluateAsync(ValkeyLua.CompareAndDelete, new RedisKey[] { ValkeyKeys.ShardOwner(id) }, new RedisValue[] { instanceId });
            await db.KeyDeleteAsync($"{ValkeyKeys.Prefix}instance:{instanceId}:shards");
        }
        catch (Exception ex) { _log.LogDebug(ex, "Shard release skipped."); }
    }
}
