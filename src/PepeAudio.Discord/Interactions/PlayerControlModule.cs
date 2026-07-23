// SPDX-License-Identifier: Apache-2.0
using Discord;
using Discord.Interactions;
using Discord.WebSocket;
using PepeAudio.Application.Playback;
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Enums;
using PepeAudio.Discord.Components;

namespace PepeAudio.Discord.Interactions;

[RequireContext(ContextType.Guild)]
public sealed class PlayerControlModule : InteractionModuleBase<ShardedInteractionContext>
{
    private readonly IPlaybackService _playback;
    private readonly NowPlayingService _now;

    public PlayerControlModule(IPlaybackService playback, NowPlayingService now)
    {
        _playback = playback;
        _now = now;
    }

    [ComponentInteraction("player:*", ignoreGroupNames: true)]
    public async Task OnControl(string action)
    {
        if (!PlayerCustomIds.TryParse(action, out var control))
        {
            await DeferAsync();
            return;
        }
        await Send(control, null);
        await RefreshAsync();
    }

    [ComponentInteraction(PlayerCustomIds.VolumeSelect, ignoreGroupNames: true)]
    public async Task OnVolume(string[] values)
    {
        if (values.Length > 0 && int.TryParse(values[0], out _))
            await Send(PlayerControl.SetVolume, values[0]);
        await RefreshAsync();
    }

    [ComponentInteraction(PlayerCustomIds.PresetSelect, ignoreGroupNames: true)]
    public async Task OnPreset(string[] values)
    {
        if (values.Length > 0)
            await Send(PlayerControl.SetPreset, values[0]);
        await RefreshAsync();
    }

    [ComponentInteraction(PlayerCustomIds.AddTrack, ignoreGroupNames: true)]
    public Task OnAddTrack()
        => Context.Interaction.RespondWithModalAsync<AddTrackModal>(PlayerCustomIds.AddTrackModal);

    [ModalInteraction(PlayerCustomIds.AddTrackModal, ignoreGroupNames: true)]
    public async Task OnAddTrackSubmit(AddTrackModal modal)
    {
        await DeferAsync(ephemeral: true);
        if (Context.User is not IGuildUser { VoiceChannel: { } voice })
        {
            await Context.Interaction.FollowupTextAsync("先にボイスチャンネルに参加してください。");
            return;
        }
        try
        {
            var req = new PlayRequest(Context.Guild.Id, voice.Id, Context.Channel.Id, Context.User.Id, modal.Query, null);
            await _playback.PlayAsync(req, CancellationToken.None);
            await _now.RefreshAsync(Context.Guild.Id);
            await Context.Interaction.FollowupTextAsync("キューに追加しました。");
        }
        catch (Exception ex)
        {
            await Context.Interaction.FollowupTextAsync($"再生できませんでした: {ex.Message}");
        }
    }

    private Task Send(PlayerControl control, string? arg)
        => _playback.ControlAsync(new ControlEnvelope(Context.Guild.Id, control, arg, Context.User.Id, 0, DateTimeOffset.UtcNow));

    private async Task RefreshAsync()
    {
        var interaction = (SocketMessageComponent)Context.Interaction;
        await interaction.UpdateAsync(m =>
        {
            m.Components = _now.BuildCard(Context.Guild.Id);
            m.Flags = MessageFlags.ComponentsV2;
        });
        _now.Track(Context.Guild.Id, interaction.Message);
    }
}
