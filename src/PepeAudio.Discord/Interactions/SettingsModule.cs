// SPDX-License-Identifier: Apache-2.0
using Discord.Interactions;
using PepeAudio.Application.Playback;

namespace PepeAudio.Discord.Interactions;

[RequireContext(ContextType.Guild)]
public sealed class SettingsModule : InteractionModuleBase<ShardedInteractionContext>
{
    private readonly IPlaybackService _playback;

    public SettingsModule(IPlaybackService playback) => _playback = playback;

    [SlashCommand("autoplay", "キューが尽きたら関連曲を自動で流します（オン/オフ切替）")]
    public async Task Autoplay()
    {
        await DeferAsync(ephemeral: true);
        var on = await _playback.ToggleAutoplayAsync(Context.Guild.Id);
        await Context.Interaction.FollowupTextAsync(on
            ? "オートプレイをオンにしました。キューが空になると関連曲を自動再生します。"
            : "オートプレイをオフにしました。");
    }
}
