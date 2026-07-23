// SPDX-License-Identifier: Apache-2.0
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using PepeAudio.Application.Playback;
using PepeAudio.Cache;
using PepeAudio.Core.Sharding;
using PepeAudio.Host.Coordination;

namespace PepeAudio.Host.Lifecycle;

// Consumes control commands routed to this instance's owned shards and applies
// them to the local players (receiving end of the unified control chain).
public sealed class ControlBusConsumerService : BackgroundService
{
    private const string Group = "pepe-control";

    private readonly ICommandBus _bus;
    private readonly IShardTopology _topology;
    private readonly InstanceIdentity _identity;
    private readonly IPlaybackService _playback;
    private readonly ILogger<ControlBusConsumerService> _log;

    public ControlBusConsumerService(ICommandBus bus, IShardTopology topology, InstanceIdentity identity,
        IPlaybackService playback, ILogger<ControlBusConsumerService> log)
    {
        _bus = bus;
        _topology = topology;
        _identity = identity;
        _playback = playback;
        _log = log;
    }

    protected override Task ExecuteAsync(CancellationToken ct)
        => Task.WhenAll(_topology.OwnedShards.Select(shard => ConsumeWithRestartAsync(shard, ct)));

    // Restart a shard's consumer if it faults, so a transient error does not silently
    // stop control delivery for that shard until the whole process is restarted.
    private async Task ConsumeWithRestartAsync(int shard, CancellationToken ct)
    {
        while (!ct.IsCancellationRequested)
        {
            try
            {
                await _bus.ConsumeAsync(shard, Group, _identity.InstanceId, env =>
                {
                    _playback.ApplyLocal(env);
                    return Task.CompletedTask;
                }, ct);
                return;
            }
            catch (OperationCanceledException) when (ct.IsCancellationRequested) { return; }
            catch (Exception ex)
            {
                _log.LogWarning(ex, "Control consumer for shard {Shard} faulted; restarting", shard);
                try { await Task.Delay(TimeSpan.FromSeconds(2), ct); } catch { return; }
            }
        }
    }
}
