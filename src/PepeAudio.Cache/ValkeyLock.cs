// SPDX-License-Identifier: Apache-2.0
using StackExchange.Redis;

namespace PepeAudio.Cache;

// Single-owner lock: SET NX PX + a unique token, released/renewed only by the
// holder (Lua compare-and-delete). Used for voice ownership and one-shot guards.
public interface IValkeyLock
{
    Task<bool> TryAcquireAsync(string key, string token, TimeSpan ttl);
    Task<bool> RenewAsync(string key, string token, TimeSpan ttl);
    Task ReleaseAsync(string key, string token);
    Task<bool> RunOnceAsync(string key, TimeSpan ttl, Func<Task> action);
}

public sealed class ValkeyLock : IValkeyLock
{
    private readonly IValkeyConnection _valkey;

    public ValkeyLock(IValkeyConnection valkey) => _valkey = valkey;

    public async Task<bool> TryAcquireAsync(string key, string token, TimeSpan ttl)
    {
        // No/unreachable coordinator: single-node dev proceeds without a lock.
        var db = _valkey.TryGetDatabase();
        if (db is null) return true;
        try { return await db.StringSetAsync(key, token, ttl, When.NotExists); }
        catch { return true; }
    }

    public async Task<bool> RenewAsync(string key, string token, TimeSpan ttl)
    {
        var db = _valkey.TryGetDatabase();
        if (db is null) return true;
        try
        {
            var result = await db.ScriptEvaluateAsync(ValkeyLua.CompareAndPExpire,
                new RedisKey[] { key }, new RedisValue[] { token, (long)ttl.TotalMilliseconds });
            return (long)result == 1;
        }
        catch { return true; }
    }

    public async Task ReleaseAsync(string key, string token)
    {
        var db = _valkey.TryGetDatabase();
        if (db is null) return;
        try { await db.ScriptEvaluateAsync(ValkeyLua.CompareAndDelete, new RedisKey[] { key }, new RedisValue[] { token }); }
        catch { /* best effort */ }
    }

    public async Task<bool> RunOnceAsync(string key, TimeSpan ttl, Func<Task> action)
    {
        var token = Guid.NewGuid().ToString("N");
        if (!await TryAcquireAsync(key, token, ttl)) return false;
        try { await action(); return true; }
        finally { await ReleaseAsync(key, token); }
    }
}
