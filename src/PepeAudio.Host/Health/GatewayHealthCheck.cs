// SPDX-License-Identifier: Apache-2.0
using Microsoft.Extensions.Diagnostics.HealthChecks;
using PepeAudio.Audio;
using PepeAudio.Host.Coordination;

namespace PepeAudio.Host.Health;

// Readiness: the bot has logged in and all owned shards report READY. Data also
// surfaces active voice sessions for capacity monitoring.
public sealed class GatewayHealthCheck : IHealthCheck
{
    private readonly BotHealthState _health;
    private readonly IPlayerManager _players;

    public GatewayHealthCheck(BotHealthState health, IPlayerManager players)
    {
        _health = health;
        _players = players;
    }

    public Task<HealthCheckResult> CheckHealthAsync(HealthCheckContext context, CancellationToken ct)
    {
        var data = new Dictionary<string, object>
        {
            ["loggedIn"] = _health.LoggedIn,
            ["readyShards"] = _health.ReadyShards,
            ["expectedShards"] = _health.ExpectedShards,
            ["activeVoices"] = _players.ActiveCount,
        };

        // Not-all-ready must FAIL readiness: Degraded maps to HTTP 200, which would let a
        // rolling deploy cut over before every owned shard is actually serving.
        var result = _health.Ready
            ? HealthCheckResult.Healthy("All owned shards ready.", data)
            : HealthCheckResult.Unhealthy(
                _health.LoggedIn ? "Some shards are not ready yet." : "Gateway not connected.", data: data);
        return Task.FromResult(result);
    }
}
