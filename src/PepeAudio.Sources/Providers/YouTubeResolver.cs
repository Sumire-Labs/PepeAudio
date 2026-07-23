// SPDX-License-Identifier: Apache-2.0
using System.Runtime.CompilerServices;
using Microsoft.Extensions.Options;
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Enums;
using PepeAudio.Core.Exceptions;
using PepeAudio.Sources.Matching;
using PepeAudio.Sources.Models;
using PepeAudio.Sources.YtDlp;

namespace PepeAudio.Sources.Providers;

// YouTube/youtu.be links (video or playlist) and bare search terms.
public sealed class YouTubeResolver : ISourceResolver
{
    private readonly IYtDlpClient _ytdlp;
    private readonly IYouTubeMatcher _matcher;
    private readonly YtDlpOptions _opt;

    public YouTubeResolver(IYtDlpClient ytdlp, IYouTubeMatcher matcher, IOptions<YtDlpOptions> opt)
    {
        _ytdlp = ytdlp;
        _matcher = matcher;
        _opt = opt.Value;
    }

    public SourceKind Kind => SourceKind.YouTube;
    public int Priority => 20;

    public bool CanHandle(ResolveRequest req)
    {
        if (req.Attachment is not null || !req.HasUrl) return false;
        if (!Uri.TryCreate(req.Url, UriKind.Absolute, out var uri)) return true; // bare search
        var host = uri.Host.ToLowerInvariant();
        return host.Contains("youtube.com") || host.Contains("youtu.be");
    }

    public async IAsyncEnumerable<PlayableRef> ResolveAsync(
        ResolveRequest req, [EnumeratorCancellation] CancellationToken ct)
    {
        var input = req.Url!;
        if (!Uri.TryCreate(input, UriKind.Absolute, out var uri))
        {
            var match = await _matcher.MatchAsync(new MatchQuery(input, "", 0, null), req.RequesterId, ct)
                ?? throw new ResolveFailedException($"'{input}' に一致する YouTube の結果がありません。");
            yield return match;
            yield break;
        }

        if (IsPlaylist(uri))
        {
            foreach (var e in await _ytdlp.PlaylistEntriesAsync(input, _opt.MaxPlaylistItems, ct))
                yield return Entry(e, req.RequesterId);
            yield break;
        }

        var resolved = await _ytdlp.GetTrackAsync(input, ct)
            ?? throw new ResolveFailedException("YouTube から再生可能なトラックを取得できませんでした。");
        var meta = resolved.Track;
        yield return new PlayableRef(SourceKind.YouTube, meta.WebpageUrl, Seekable: true,
            new TrackInfo(meta.Title, meta.Artist, SourceKind.YouTube, meta.WebpageUrl, meta.DurationMs, meta.Thumbnail, false, req.RequesterId),
            NeedsResolution: true, Prefetched: resolved.StreamUrl);
    }

    private static PlayableRef Entry(YtDlpCandidate e, ulong requester)
        => new(SourceKind.YouTube, e.WebpageUrl, Seekable: true,
            new TrackInfo(e.Title, e.Channel ?? "YouTube", SourceKind.YouTube, e.WebpageUrl, e.DurationMs, YouTubeThumbnail.For(e.Id), false, requester),
            NeedsResolution: true);

    private static bool IsPlaylist(Uri uri)
    {
        if (uri.AbsolutePath.Contains("playlist", StringComparison.OrdinalIgnoreCase)) return true;
        var q = uri.Query;
        return q.Contains("list=", StringComparison.OrdinalIgnoreCase) && !q.Contains("v=", StringComparison.OrdinalIgnoreCase);
    }
}
