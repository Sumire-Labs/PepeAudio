// SPDX-License-Identifier: Apache-2.0
using System.Collections.Concurrent;
using Discord.WebSocket;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using PepeAudio.Audio;

namespace PepeAudio.Host.Lifecycle;

// Watches voice-state changes so the bot doesn't squat in a channel: it leaves a short while
// after the last human departs, follows the connection if a mod drags it, and tears down if it
// is kicked. The empty-queue idle timeout lives inside GuildPlayer; this covers empty channels.
public sealed class VoicePresenceService : IHostedService
{
    private static readonly TimeSpan AloneTimeout = TimeSpan.FromSeconds(60);
    private readonly DiscordShardedClient _client;
    private readonly IPlayerManager _players;
    private readonly ILogger<VoicePresenceService> _log;
    private readonly ConcurrentDictionary<ulong, CancellationTokenSource> _aloneTimers = new();

    public VoicePresenceService(DiscordShardedClient client, IPlayerManager players, ILogger<VoicePresenceService> log)
    {
        _client = client;
        _players = players;
        _log = log;
    }

    public Task StartAsync(CancellationToken ct)
    {
        _client.UserVoiceStateUpdated += OnVoiceStateUpdated;
        return Task.CompletedTask;
    }

    public Task StopAsync(CancellationToken ct)
    {
        _client.UserVoiceStateUpdated -= OnVoiceStateUpdated;
        return Task.CompletedTask;
    }

    private Task OnVoiceStateUpdated(SocketUser user, SocketVoiceState before, SocketVoiceState after)
    {
        var guildId = (after.VoiceChannel ?? before.VoiceChannel)?.Guild.Id;
        if (guildId is not { } gid || !_players.TryGet(gid, out var player) || player is null)
            return Task.CompletedTask;

        if (_client.CurrentUser is { } me && user.Id == me.Id)
        {
            if (after.VoiceChannel is null) { CancelAlone(gid); _ = _players.RemoveAsync(gid); }
            else if (after.VoiceChannel.Id != player.VoiceChannelId) player.SetVoiceChannel(after.VoiceChannel.Id);
            return Task.CompletedTask;
        }

        var channel = _client.GetGuild(gid)?.GetVoiceChannel(player.VoiceChannelId);
        if (channel is null) return Task.CompletedTask;
        if (channel.ConnectedUsers.Any(u => !u.IsBot)) CancelAlone(gid);
        else ArmAlone(gid);
        return Task.CompletedTask;
    }

    private void ArmAlone(ulong guildId)
    {
        var cts = new CancellationTokenSource();
        if (!_aloneTimers.TryAdd(guildId, cts)) { cts.Dispose(); return; }
        _ = Task.Run(async () =>
        {
            try { await Task.Delay(AloneTimeout, cts.Token); }
            catch { return; }
            _aloneTimers.TryRemove(guildId, out _);
            _log.LogInformation("Voice channel empty in guild {Guild} for {Sec}s; leaving", guildId, AloneTimeout.TotalSeconds);
            try { await _players.RemoveAsync(guildId); } catch { /* already gone */ }
        });
    }

    private void CancelAlone(ulong guildId)
    {
        if (_aloneTimers.TryRemove(guildId, out var cts)) cts.Cancel();
    }
}
