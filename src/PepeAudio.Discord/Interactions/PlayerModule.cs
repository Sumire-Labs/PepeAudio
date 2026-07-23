// SPDX-License-Identifier: Apache-2.0
using Discord;
using Discord.Interactions;
using PepeAudio.Application.Playback;
using PepeAudio.Discord.Components;

namespace PepeAudio.Discord.Interactions;

[RequireContext(ContextType.Guild)]
public sealed class PlayerModule : InteractionModuleBase<ShardedInteractionContext>
{
    private readonly IPlaybackService _playback;
    private readonly NowPlayingService _now;

    public PlayerModule(IPlaybackService playback, NowPlayingService now)
    {
        _playback = playback;
        _now = now;
    }

    [SlashCommand("now", "音楽プレイヤーを表示します")]
    public async Task Now()
    {
        await RespondAsync(components: _now.BuildCard(Context.Guild.Id), flags: MessageFlags.ComponentsV2);
        await _now.ReplaceAsync(Context.Guild.Id, await GetOriginalResponseAsync());
    }

    [SlashCommand("quit", "ボイスチャンネルから退出し、キューをクリアします")]
    public async Task Quit()
    {
        await DeferAsync(ephemeral: true);
        await _playback.QuitAsync(Context.Guild.Id);
        _now.Forget(Context.Guild.Id);
        await Context.Interaction.FollowupTextAsync("ボイスチャンネルから退出し、キューをクリアしました。");
    }
}
