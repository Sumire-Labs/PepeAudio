// SPDX-License-Identifier: Apache-2.0
using System.Collections.Concurrent;
using Discord.WebSocket;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.SignalR;
using Microsoft.Extensions.Caching.Memory;
using PepeAudio.Application.Playback;
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Enums;
using PepeAudio.Web.Auth;

namespace PepeAudio.Web.Hubs;

[Authorize]
public sealed class PlayerHub : Hub
{
    // Inner map used as a thread-safe set: a background broadcaster enumerates it while
    // hub methods mutate it, so a plain HashSet would throw / tear.
    private static readonly ConcurrentDictionary<string, ConcurrentDictionary<string, byte>> ConnectionGuilds = new();

    private readonly IPlaybackService _playback;
    private readonly DiscordShardedClient _client;
    private readonly IMemoryCache _cache;

    public PlayerHub(IPlaybackService playback, DiscordShardedClient client, IMemoryCache cache)
    {
        _playback = playback;
        _client = client;
        _cache = cache;
    }

    public static string Group(string guildId) => $"guild:{guildId}";
    public static IReadOnlyCollection<string> ActiveGuilds()
        => ConnectionGuilds.Values.SelectMany(s => s.Keys).Distinct().ToArray();

    public async Task Subscribe(string guildId)
    {
        if (!CanManage(guildId) || !ulong.TryParse(guildId, out var id))
            throw new HubException("アクセスが拒否されました。");
        await Groups.AddToGroupAsync(Context.ConnectionId, Group(guildId));
        ConnectionGuilds.GetOrAdd(Context.ConnectionId, _ => new()).TryAdd(guildId, 0);
        await Clients.Caller.SendAsync("PlayerState", _playback.GetState(id));
    }

    public async Task Control(string guildId, string action)
    {
        if (!Enum.TryParse<PlayerControl>(action, ignoreCase: true, out var control)) return;
        await SendControl(guildId, control, null);
    }

    public Task ReorderQueue(string guildId, int from, int to)
        => SendControl(guildId, PlayerControl.ReorderQueue, $"{from}:{to}");

    public Task RemoveTrack(string guildId, int index)
        => SendControl(guildId, PlayerControl.RemoveTrack, index.ToString());

    private async Task SendControl(string guildId, PlayerControl control, string? arg)
    {
        if (!CanManage(guildId) || !ulong.TryParse(guildId, out var id)) return;
        await _playback.ControlAsync(new ControlEnvelope(id, control, arg, UserId(), 0, DateTimeOffset.UtcNow));
        await Clients.Group(Group(guildId)).SendAsync("PlayerState", _playback.GetState(id));
    }

    public async Task Play(string guildId, string url)
    {
        if (!CanManage(guildId) || !ulong.TryParse(guildId, out var id))
            throw new HubException("アクセスが拒否されました。");
        var voice = _client.GetGuild(id)?.GetUser(UserId())?.VoiceChannel
            ?? throw new HubException("先にボイスチャンネルに参加してください。");
        await _playback.PlayAsync(new PlayRequest(id, voice.Id, 0, UserId(), url, null), Context.ConnectionAborted);
        await Clients.Group(Group(guildId)).SendAsync("PlayerState", _playback.GetState(id));
    }

    public override Task OnDisconnectedAsync(Exception? exception)
    {
        ConnectionGuilds.TryRemove(Context.ConnectionId, out _);
        return base.OnDisconnectedAsync(exception);
    }

    private ulong UserId() => ulong.TryParse(Context.User?.FindFirst("sub")?.Value, out var id) ? id : 0;

    private bool CanManage(string guildId) => GuildAccess.CanManage(Context.User, _cache, guildId);
}
