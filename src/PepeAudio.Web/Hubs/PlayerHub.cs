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
using PepeAudio.Web.Realtime;

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

    // No-arg controls the dashboard buttons map to. Arg'd controls (loop/volume/seek/queue
    // mutations) have their own strongly-typed methods below.
    private static readonly HashSet<PlayerControl> NoArgControls = new()
    {
        PlayerControl.PlayPause, PlayerControl.Skip, PlayerControl.Previous,
        PlayerControl.Stop, PlayerControl.Shuffle, PlayerControl.ToggleAura, PlayerControl.ClearQueue,
    };

    public async Task Subscribe(string guildId)
    {
        if (!CanManage(guildId) || !ulong.TryParse(guildId, out var id))
            throw new HubException("アクセスが拒否されました。");
        await Groups.AddToGroupAsync(Context.ConnectionId, Group(guildId));
        ConnectionGuilds.GetOrAdd(Context.ConnectionId, _ => new()).TryAdd(guildId, 0);
        await Clients.Caller.SendAsync("PlayerState", Snap(id));
    }

    public Task Control(string guildId, string action)
        => Enum.TryParse<PlayerControl>(action, ignoreCase: true, out var control) && NoArgControls.Contains(control)
            ? SendControl(guildId, control, null)
            : Task.CompletedTask;

    public Task SetLoop(string guildId, string mode) => SendControl(guildId, PlayerControl.Loop, mode);
    public Task SetVolume(string guildId, int percent) => SendControl(guildId, PlayerControl.SetVolume, percent.ToString());
    public Task Seek(string guildId, long positionMs) => SendControl(guildId, PlayerControl.Seek, positionMs.ToString());
    public Task MoveTrack(string guildId, string id, int toIndex) => SendControl(guildId, PlayerControl.ReorderQueue, $"{id}:{toIndex}");
    public Task RemoveTrack(string guildId, string id) => SendControl(guildId, PlayerControl.RemoveTrack, id);
    public Task JumpTo(string guildId, string id) => SendControl(guildId, PlayerControl.JumpTo, id);

    public async Task SetAutoplay(string guildId, bool enabled)
    {
        if (!CanManage(guildId) || !ulong.TryParse(guildId, out var id)) return;
        await _playback.SetAutoplayAsync(id, enabled);
        await Clients.Group(Group(guildId)).SendAsync("PlayerState", Snap(id));
    }

    private async Task SendControl(string guildId, PlayerControl control, string? arg)
    {
        if (!CanManage(guildId) || !ulong.TryParse(guildId, out var id)) return;
        await _playback.ControlAsync(new ControlEnvelope(id, control, arg, UserId(), 0, DateTimeOffset.UtcNow));
        await Clients.Group(Group(guildId)).SendAsync("PlayerState", Snap(id));
    }

    // Enqueue a URL or search term. Requires the caller to be in a voice channel (that's the
    // target the bot joins); resolution + capacity checks happen inside PlaybackService.
    public async Task Play(string guildId, string query)
    {
        if (!CanManage(guildId) || !ulong.TryParse(guildId, out var id))
            throw new HubException("アクセスが拒否されました。");
        if (string.IsNullOrWhiteSpace(query)) return;
        var voice = _client.GetGuild(id)?.GetUser(UserId())?.VoiceChannel
            ?? throw new HubException("先にボイスチャンネルに参加してください。");
        await _playback.PlayAsync(new PlayRequest(id, voice.Id, 0, UserId(), query, null), Context.ConnectionAborted);
        await Clients.Group(Group(guildId)).SendAsync("PlayerState", Snap(id));
    }

    private PlayerSnapshotDto Snap(ulong id) => PlayerSnapshot.From(_playback.GetState(id), _client);

    public override Task OnDisconnectedAsync(Exception? exception)
    {
        ConnectionGuilds.TryRemove(Context.ConnectionId, out _);
        return base.OnDisconnectedAsync(exception);
    }

    private ulong UserId() => ulong.TryParse(Context.User?.FindFirst("sub")?.Value, out var id) ? id : 0;

    private bool CanManage(string guildId) => GuildAccess.CanManage(Context.User, _cache, guildId);
}
