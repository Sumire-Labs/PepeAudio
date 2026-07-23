// SPDX-License-Identifier: Apache-2.0
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using PepeAudio.Application;
using PepeAudio.Audio;

namespace PepeAudio.Host.Lifecycle;

// On SIGTERM: stop accepting new /play, then tear down active voice sessions
// cleanly (disconnect + release locks) while the gateway is still connected.
// Registered last so its StopAsync runs first, before the client logs out.
public sealed class GracefulDrainService : IHostedService
{
    private readonly ShutdownState _shutdown;
    private readonly IPlayerManager _players;
    private readonly ILogger<GracefulDrainService> _log;

    public GracefulDrainService(ShutdownState shutdown, IPlayerManager players, ILogger<GracefulDrainService> log)
    {
        _shutdown = shutdown;
        _players = players;
        _log = log;
    }

    public Task StartAsync(CancellationToken cancellationToken) => Task.CompletedTask;

    public async Task StopAsync(CancellationToken cancellationToken)
    {
        _shutdown.BeginDraining();
        _log.LogInformation("Draining {Count} active voice session(s)…", _players.ActiveCount);
        await _players.DrainAllAsync();
    }
}
