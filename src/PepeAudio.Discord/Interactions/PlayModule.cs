// SPDX-License-Identifier: Apache-2.0
using Discord;
using Discord.Interactions;
using PepeAudio.Application.Playback;
using PepeAudio.Discord.Components;

namespace PepeAudio.Discord.Interactions;

[RequireContext(ContextType.Guild)]
public sealed class PlayModule : InteractionModuleBase<ShardedInteractionContext>
{
    private readonly IPlaybackService _playback;
    private readonly NowPlayingService _now;

    public PlayModule(IPlaybackService playback, NowPlayingService now)
    {
        _playback = playback;
        _now = now;
    }

    [SlashCommand("play", "URL またはアップロードした音声ファイルを再生します")]
    public async Task Play(
        [Summary("url", "曲・プレイリストの URL、または検索ワード")] string? url = null,
        [Summary("file", "再生する音声ファイル")] IAttachment? file = null)
    {
        await DeferAsync();

        if (url is null && file is null)
        {
            await Context.Interaction.FollowupTextAsync("URL を指定するか、音声ファイルを添付してください。");
            return;
        }
        if (url is not null && file is not null)
        {
            await Context.Interaction.FollowupTextAsync("URL とファイルは、どちらか一方だけを指定してください。");
            return;
        }
        if (Context.User is not IGuildUser { VoiceChannel: { } voice })
        {
            await Context.Interaction.FollowupTextAsync("先にボイスチャンネルに参加してください。");
            return;
        }

        try
        {
            var upload = file is null ? null : new UploadedFile(file.Url, file.Filename, file.ContentType, file.Size);
            var req = new PlayRequest(Context.Guild.Id, voice.Id, Context.Channel.Id, Context.User.Id, url, upload);
            await _playback.PlayAsync(req, CancellationToken.None);

            // Update the existing player card in place; otherwise post a fresh one and track it.
            if (_now.HasCard(Context.Guild.Id))
            {
                await _now.RefreshAsync(Context.Guild.Id);
                await Context.Interaction.FollowupTextAsync("キューに追加しました。");
            }
            else
            {
                var msg = await FollowupAsync(components: _now.BuildCard(Context.Guild.Id), flags: MessageFlags.ComponentsV2);
                _now.Track(Context.Guild.Id, msg);
            }
        }
        catch (Exception ex)
        {
            await Context.Interaction.FollowupTextAsync($"再生できませんでした: {ex.Message}");
        }
    }
}
