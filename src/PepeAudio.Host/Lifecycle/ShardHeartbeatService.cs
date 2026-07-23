// SPDX-License-Identifier: Apache-2.0
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using PepeAudio.Cache;
using PepeAudio.Core.Sharding;
using PepeAudio.Host.Coordination;

namespace PepeAudio.Host.Lifecycle;

// Registers this instance's shard ownership in Valkey and renews the heartbeat,
// releasing ownership on shutdown so a replacement can reclaim it.
public sealed class ShardHeartbeatService : BackgroundService
{
    private static readonly TimeSpan Ttl = TimeSpan.FromSeconds(30);
    private static readonly TimeSpan Interval = TimeSpan.FromSeconds(10);

    private readonly IShardRegistry _registry;
    private readonly IShardTopology _topology;
    private readonly InstanceIdentity _identity;
    private readonly ILogger<ShardHeartbeatService> _log;

    public ShardHeartbeatService(IShardRegistry registry, IShardTopology topology,
        InstanceIdentity identity, ILogger<ShardHeartbeatService> log)
    {
        _registry = registry;
        _topology = topology;
        _identity = identity;
        _log = log;
    }

    protected override async Task ExecuteAsync(CancellationToken ct)
    {
        var shards = _topology.OwnedShards.ToArray();
        await _registry.RegisterAsync(shards, _identity.InstanceId, Ttl);
        _log.LogInformation("Instance {Id} owns shards [{Shards}]", _identity.InstanceId, string.Join(",", shards));

        try
        {
            using var timer = new PeriodicTimer(Interval);
            while (await timer.WaitForNextTickAsync(ct))
                await _registry.RenewAsync(shards, _identity.InstanceId, Ttl);
        }
        catch (OperationCanceledException) { /* shutting down */ }
        finally
        {
            await _registry.ReleaseAsync(shards, _identity.InstanceId);
        }
    }
}
