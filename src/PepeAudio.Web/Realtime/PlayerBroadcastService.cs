// SPDX-License-Identifier: Apache-2.0
using Discord.WebSocket;
using Microsoft.AspNetCore.SignalR;
using Microsoft.Extensions.Hosting;
using PepeAudio.Application.Playback;
using PepeAudio.Web.Hubs;

namespace PepeAudio.Web.Realtime;

// Pushes player state to subscribed guild groups on a slow tick (control invokes
// push immediately from the hub; this covers passive progress updates).
public sealed class PlayerBroadcastService : BackgroundService
{
    private static readonly TimeSpan Interval = TimeSpan.FromSeconds(2);

    private readonly IHubContext<PlayerHub> _hub;
    private readonly IPlaybackService _playback;
    private readonly DiscordShardedClient _client;

    public PlayerBroadcastService(IHubContext<PlayerHub> hub, IPlaybackService playback, DiscordShardedClient client)
    {
        _hub = hub;
        _playback = playback;
        _client = client;
    }

    protected override async Task ExecuteAsync(CancellationToken ct)
    {
        using var timer = new PeriodicTimer(Interval);
        while (await timer.WaitForNextTickAsync(ct))
        {
            foreach (var guildId in PlayerHub.ActiveGuilds())
            {
                if (!ulong.TryParse(guildId, out var id)) continue;
                var dto = PlayerSnapshot.From(_playback.GetState(id), _client);
                await _hub.Clients.Group(PlayerHub.Group(guildId)).SendAsync("PlayerState", dto, ct);
            }
        }
    }
}
