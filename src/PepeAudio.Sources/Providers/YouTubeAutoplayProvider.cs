// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Enums;
using PepeAudio.Sources.YtDlp;

namespace PepeAudio.Sources.Providers;

// Radio autoplay: expands YouTube's "RD" mix for the seed video into related tracks.
public sealed class YouTubeAutoplayProvider : IAutoplayProvider
{
    private readonly IYtDlpClient _ytdlp;

    public YouTubeAutoplayProvider(IYtDlpClient ytdlp) => _ytdlp = ytdlp;

    public async Task<IReadOnlyList<PlayableRef>> RelatedAsync(PlayableRef seed, int max, CancellationToken ct)
    {
        if (seed.Source != SourceKind.YouTube) return Array.Empty<PlayableRef>();
        var id = VideoId(seed.Info.Url) ?? VideoId(seed.Input);
        if (id is null) return Array.Empty<PlayableRef>();

        var mix = $"https://www.youtube.com/watch?v={id}&list=RD{id}";
        var entries = await _ytdlp.PlaylistEntriesAsync(mix, max + 1, ct);
        var requester = seed.Info.RequestedBy;
        return entries
            .Where(e => !string.Equals(e.Id, id, StringComparison.OrdinalIgnoreCase))
            .Take(max)
            .Select(e => new PlayableRef(SourceKind.YouTube, e.WebpageUrl, Seekable: true,
                new TrackInfo(e.Title, e.Channel ?? "YouTube", SourceKind.YouTube, e.WebpageUrl, e.DurationMs, YouTubeThumbnail.For(e.Id), false, requester),
                NeedsResolution: true))
            .ToList();
    }

    private static string? VideoId(string url)
    {
        if (!Uri.TryCreate(url, UriKind.Absolute, out var uri)) return null;
        if (uri.Host.Contains("youtu.be", StringComparison.OrdinalIgnoreCase))
            return uri.AbsolutePath.Trim('/') is { Length: > 0 } p ? p : null;
        foreach (var part in uri.Query.TrimStart('?').Split('&', StringSplitOptions.RemoveEmptyEntries))
            if (part.StartsWith("v=", StringComparison.Ordinal)) return part[2..];
        return null;
    }
}
