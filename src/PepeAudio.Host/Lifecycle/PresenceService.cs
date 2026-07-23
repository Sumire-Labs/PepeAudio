// SPDX-License-Identifier: Apache-2.0
using System.Diagnostics;
using System.Globalization;
using Discord;
using Discord.WebSocket;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using PepeAudio.Host.Coordination;

namespace PepeAudio.Host.Lifecycle;

// Shows the process memory footprint (RSS) as the bot's activity, refreshed periodically and
// re-applied each tick so it survives shard reconnects. Ticks are skipped until every shard is
// ready (SetActivity silently no-ops before READY); bot accounts cannot use a custom status.
public sealed class PresenceService : BackgroundService
{
    private static readonly TimeSpan Interval = TimeSpan.FromSeconds(30);
    private readonly DiscordShardedClient _client;
    private readonly BotHealthState _health;
    private readonly ILogger<PresenceService> _log;

    public PresenceService(DiscordShardedClient client, BotHealthState health, ILogger<PresenceService> log)
    {
        _client = client;
        _health = health;
        _log = log;
    }

    protected override async Task ExecuteAsync(CancellationToken ct)
    {
        using var timer = new PeriodicTimer(Interval);
        while (await timer.WaitForNextTickAsync(ct))
        {
            if (!_health.Ready) continue;
            try { await _client.SetActivityAsync(new Game($"メモリ: {MemoryMb()} MB", ActivityType.Watching)); }
            catch (Exception ex) { _log.LogDebug(ex, "Presence update skipped"); }
        }
    }

    private static string MemoryMb()
    {
        using var p = Process.GetCurrentProcess();
        return (p.WorkingSet64 / 1024d / 1024d).ToString("0.0", CultureInfo.InvariantCulture);
    }
}
