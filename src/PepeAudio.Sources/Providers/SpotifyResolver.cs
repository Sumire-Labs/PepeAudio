// SPDX-License-Identifier: Apache-2.0
using System.Runtime.CompilerServices;
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Enums;
using PepeAudio.Core.Exceptions;
using PepeAudio.Sources.Cache;
using PepeAudio.Sources.Matching;
using PepeAudio.Sources.Metadata;
using PepeAudio.Sources.Models;

namespace PepeAudio.Sources.Providers;

// Spotify links: metadata only (DRM), matched to YouTube via the scorer + cache.
public sealed class SpotifyResolver : ISourceResolver
{
    private readonly SpotifyMetadataClient _spotify;
    private readonly SpotifyPublicClient _spotifyPublic;
    private readonly IYouTubeMatcher _matcher;
    private readonly ITrackCache _cache;

    public SpotifyResolver(SpotifyMetadataClient spotify, SpotifyPublicClient spotifyPublic,
        IYouTubeMatcher matcher, ITrackCache cache)
    {
        _spotify = spotify;
        _spotifyPublic = spotifyPublic;
        _matcher = matcher;
        _cache = cache;
    }

    public SourceKind Kind => SourceKind.Spotify;
    public int Priority => 25;

    public bool CanHandle(ResolveRequest req)
        => req.Attachment is null && req.HasUrl && SpotifyUrl.IsSpotify(req.Url);

    public async IAsyncEnumerable<PlayableRef> ResolveAsync(
        ResolveRequest req, [EnumeratorCancellation] CancellationToken ct)
    {
        if (!SpotifyUrl.TryParse(req.Url, out var reference))
            throw new ResolveFailedException("認識できない Spotify の URL です。");

        // Fall back to credential-free public metadata when no API key is configured.
        var queries = _spotify.Enabled
            ? await _spotify.ResolveAsync(reference, ct)
            : await _spotifyPublic.ResolveAsync(reference, ct);

        await foreach (var track in MetadataMatch.MatchAndCacheAsync(
            queries, "spotify", _matcher, _cache, req.RequesterId, ct))
            yield return track;
    }
}
