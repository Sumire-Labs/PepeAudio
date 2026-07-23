// SPDX-License-Identifier: Apache-2.0
using Microsoft.Extensions.Options;
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Enums;
using PepeAudio.Sources.YtDlp;

namespace PepeAudio.Sources.Matching;

// Turns a metadata-only MatchQuery (from Spotify/Apple or a bare search) into a
// playable YouTube reference by searching and scoring candidates.
public interface IYouTubeMatcher
{
    Task<PlayableRef?> MatchAsync(MatchQuery query, ulong requesterId, CancellationToken ct);
}

public sealed class YouTubeMatcher : IYouTubeMatcher
{
    private readonly IYtDlpClient _ytdlp;
    private readonly int _searchCount;

    public YouTubeMatcher(IYtDlpClient ytdlp, IOptions<YtDlpOptions> opt)
    {
        _ytdlp = ytdlp;
        _searchCount = Math.Max(1, opt.Value.SearchCount);
    }

    public async Task<PlayableRef?> MatchAsync(MatchQuery query, ulong requesterId, CancellationToken ct)
    {
        var terms = string.IsNullOrWhiteSpace(query.Artist) ? query.Title : $"{query.Artist} {query.Title}";
        var candidates = await _ytdlp.SearchAsync(terms, _searchCount, ct);
        var best = YouTubeMatchScorer.Best(query, candidates);
        if (best is null) return null;

        var duration = query.DurationMs > 0 ? query.DurationMs : best.DurationMs;
        var info = new TrackInfo(
            string.IsNullOrWhiteSpace(query.Title) ? best.Title : query.Title,
            string.IsNullOrWhiteSpace(query.Artist) ? (best.Channel ?? "YouTube") : query.Artist,
            SourceKind.YouTube, best.WebpageUrl, duration, YouTubeThumbnail.For(best.Id), IsLive: false, requesterId);
        return new PlayableRef(SourceKind.YouTube, best.WebpageUrl, Seekable: true, info, NeedsResolution: true);
    }
}
