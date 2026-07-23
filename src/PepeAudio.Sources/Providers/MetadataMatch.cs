// SPDX-License-Identifier: Apache-2.0
using System.Runtime.CompilerServices;
using PepeAudio.Core.Contracts;
using PepeAudio.Sources.Cache;
using PepeAudio.Sources.Matching;

namespace PepeAudio.Sources.Providers;

// Shared tail for the DRM metadata providers (Spotify/Apple): resolve each query to a
// YouTube track via the scorer, backed by the two-tier cache (ISRC key when present).
internal static class MetadataMatch
{
    public static async IAsyncEnumerable<PlayableRef> MatchAndCacheAsync(
        IReadOnlyList<MatchQuery> queries, string prefix, IYouTubeMatcher matcher,
        ITrackCache cache, ulong requesterId, [EnumeratorCancellation] CancellationToken ct)
    {
        foreach (var query in queries)
        {
            var key = query.Isrc is { Length: > 0 } isrc
                ? CacheKeys.ForIsrc(isrc)
                : CacheKeys.For(prefix, query.Title + query.Artist);
            var cached = await cache.GetAsync(key, requesterId, ct);
            if (cached is not null) { yield return cached; continue; }

            var matched = await matcher.MatchAsync(query, requesterId, ct);
            if (matched is null) continue;
            await cache.SetAsync(key, matched, ct);
            yield return matched;
        }
    }
}
