// SPDX-License-Identifier: Apache-2.0
using Microsoft.Extensions.Hosting;
using PepeAudio.Audio;
using PepeAudio.Discord.Components;

namespace PepeAudio.Host.Lifecycle;

// Re-renders each active guild's live player card every 10s so the seek bar advances.
public sealed class PlayerCardUpdater : BackgroundService
{
    private static readonly TimeSpan Interval = TimeSpan.FromSeconds(10);
    private readonly NowPlayingService _now;
    private readonly IPlayerManager _players;

    public PlayerCardUpdater(NowPlayingService now, IPlayerManager players)
    {
        _now = now;
        _players = players;
    }

    protected override async Task ExecuteAsync(CancellationToken ct)
    {
        using var timer = new PeriodicTimer(Interval);
        while (await timer.WaitForNextTickAsync(ct))
            foreach (var guild in _players.ActiveGuildIds)
                await _now.RefreshAsync(guild);
    }
}
