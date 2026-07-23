// SPDX-License-Identifier: Apache-2.0
using System.Text.Json;
using Microsoft.Extensions.Logging;
using PepeAudio.Core.Contracts;
using StackExchange.Redis;

namespace PepeAudio.Cache;

// Cross-process control delivery. Commands for a guild go to its owning shard's
// Valkey Stream; the owner consumes them (consumer group = at-least-once).
public interface ICommandBus
{
    Task PublishAsync(int shardId, ControlEnvelope envelope);
    Task ConsumeAsync(int shardId, string group, string consumer, Func<ControlEnvelope, Task> handler, CancellationToken ct);
}

public sealed class ValkeyCommandBus : ICommandBus
{
    private const string Field = "e";
    private const int MaxLen = 1000;

    private readonly IValkeyConnection _valkey;
    private readonly ILogger<ValkeyCommandBus> _log;

    public ValkeyCommandBus(IValkeyConnection valkey, ILogger<ValkeyCommandBus> log)
    {
        _valkey = valkey;
        _log = log;
    }

    public async Task PublishAsync(int shardId, ControlEnvelope envelope)
    {
        var db = _valkey.TryGetDatabase();
        if (db is null) return;
        try
        {
            await db.StreamAddAsync(ValkeyKeys.ControlStream(shardId), Field, JsonSerializer.Serialize(envelope),
                maxLength: MaxLen, useApproximateMaxLength: true);
        }
        catch (Exception ex) { _log.LogWarning(ex, "Control publish to shard {Shard} failed", shardId); }
    }

    public async Task ConsumeAsync(int shardId, string group, string consumer,
        Func<ControlEnvelope, Task> handler, CancellationToken ct)
    {
        var db = _valkey.TryGetDatabase();
        if (db is null) return;
        var key = ValkeyKeys.ControlStream(shardId);
        try { await EnsureGroupAsync(db, key, group); }
        catch (Exception ex) { _log.LogDebug(ex, "Control group init skipped for shard {Shard}", shardId); return; }

        while (!ct.IsCancellationRequested)
        {
            StreamEntry[] entries;
            try { entries = await db.StreamReadGroupAsync(key, group, consumer, ">", count: 10); }
            catch (Exception ex) { _log.LogDebug(ex, "Control stream read failed for shard {Shard}", shardId); entries = Array.Empty<StreamEntry>(); }

            if (entries.Length == 0) { await Task.Delay(250, ct); continue; }

            foreach (var entry in entries)
            {
                try
                {
                    var json = entry[Field];
                    if (json.HasValue && JsonSerializer.Deserialize<ControlEnvelope>((string)json!) is { } env)
                        await handler(env);
                }
                catch (Exception ex) { _log.LogWarning(ex, "Control envelope handling failed"); }
            }
            try { await db.StreamAcknowledgeAsync(key, group, entries.Select(e => e.Id).ToArray()); }
            catch (Exception ex) { _log.LogDebug(ex, "Control ack failed for shard {Shard}; entries stay pending", shardId); }
        }
    }

    private static async Task EnsureGroupAsync(IDatabase db, string key, string group)
    {
        try { await db.StreamCreateConsumerGroupAsync(key, group, StreamPosition.NewMessages, createStream: true); }
        catch (RedisServerException ex) when (ex.Message.Contains("BUSYGROUP")) { /* already exists */ }
    }
}
