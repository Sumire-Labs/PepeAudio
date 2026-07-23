// SPDX-License-Identifier: Apache-2.0
using System.Runtime.CompilerServices;
using Microsoft.Extensions.Options;
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Enums;
using PepeAudio.Core.Exceptions;
using PepeAudio.Sources.Models;
using PepeAudio.Sources.YtDlp;

namespace PepeAudio.Sources.Providers;

// SoundCloud tracks and sets, extracted via yt-dlp (directly playable).
public sealed class SoundCloudResolver : ISourceResolver
{
    private readonly IYtDlpClient _ytdlp;
    private readonly YtDlpOptions _opt;

    public SoundCloudResolver(IYtDlpClient ytdlp, IOptions<YtDlpOptions> opt)
    {
        _ytdlp = ytdlp;
        _opt = opt.Value;
    }

    public SourceKind Kind => SourceKind.SoundCloud;
    public int Priority => 20;

    public bool CanHandle(ResolveRequest req)
        => req.Attachment is null && req.HasUrl
           && Uri.TryCreate(req.Url, UriKind.Absolute, out var uri)
           && uri.Host.Contains("soundcloud.com", StringComparison.OrdinalIgnoreCase);

    public async IAsyncEnumerable<PlayableRef> ResolveAsync(
        ResolveRequest req, [EnumeratorCancellation] CancellationToken ct)
    {
        var input = req.Url!;
        if (input.Contains("/sets/", StringComparison.OrdinalIgnoreCase))
        {
            foreach (var e in await _ytdlp.PlaylistEntriesAsync(input, _opt.MaxPlaylistItems, ct))
                yield return new PlayableRef(SourceKind.SoundCloud, e.WebpageUrl, Seekable: true,
                    new TrackInfo(e.Title, e.Channel ?? "SoundCloud", SourceKind.SoundCloud, e.WebpageUrl, e.DurationMs, null, false, req.RequesterId),
                    NeedsResolution: true);
            yield break;
        }

        var resolved = await _ytdlp.GetTrackAsync(input, ct)
            ?? throw new ResolveFailedException("SoundCloud から再生可能なトラックを取得できませんでした。");
        var meta = resolved.Track;
        yield return new PlayableRef(SourceKind.SoundCloud, meta.WebpageUrl, Seekable: true,
            new TrackInfo(meta.Title, meta.Artist, SourceKind.SoundCloud, meta.WebpageUrl, meta.DurationMs, meta.Thumbnail, false, req.RequesterId),
            NeedsResolution: true, Prefetched: resolved.StreamUrl);
    }
}
