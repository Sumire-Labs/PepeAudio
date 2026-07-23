// SPDX-License-Identifier: Apache-2.0
using System.Collections.Concurrent;
using Discord;
using PepeAudio.Application.Playback;

namespace PepeAudio.Discord.Components;

// Tracks the live player card per guild and re-renders it (title, seek bar, states) on
// demand or on a timer. Holding the IUserMessage lets us edit in place with no REST fetch.
public sealed class NowPlayingService
{
    private readonly IPlaybackService _playback;
    private readonly ConcurrentDictionary<ulong, IUserMessage> _cards = new();

    public NowPlayingService(IPlaybackService playback) => _playback = playback;

    public MessageComponent BuildCard(ulong guildId)
        => PlayerComponentFactory.Build(_playback.GetState(guildId), _playback.PresetNames);

    public void Track(ulong guildId, IUserMessage message) => _cards[guildId] = message;

    public bool HasCard(ulong guildId) => _cards.ContainsKey(guildId);

    public void Forget(ulong guildId) => _cards.TryRemove(guildId, out _);

    public async Task RefreshAsync(ulong guildId)
    {
        if (!_cards.TryGetValue(guildId, out var msg)) return;
        var state = _playback.GetState(guildId);
        // Playback finished (nothing playing and queue drained): remove the panel entirely.
        if (state.Current is null && state.Queue.Count == 0)
        {
            await DeleteAsync(guildId);
            return;
        }
        try
        {
            await msg.ModifyAsync(m =>
            {
                m.Components = PlayerComponentFactory.Build(state, _playback.PresetNames);
                m.Flags = MessageFlags.ComponentsV2;
            });
        }
        catch { _cards.TryRemove(guildId, out _); }
    }

    // Track the new panel and delete the previous one (send-new-then-delete-old so a button
    // pressed mid-swap still hits a live message). Used by /now to keep exactly one panel.
    public async Task ReplaceAsync(ulong guildId, IUserMessage message)
    {
        _cards.TryRemove(guildId, out var old);
        _cards[guildId] = message;
        if (old is not null && old.Id != message.Id)
            try { await old.DeleteAsync(); } catch { /* already gone */ }
    }

    public async Task DeleteAsync(ulong guildId)
    {
        if (!_cards.TryRemove(guildId, out var msg)) return;
        try { await msg.DeleteAsync(); } catch { /* already gone */ }
    }
}
