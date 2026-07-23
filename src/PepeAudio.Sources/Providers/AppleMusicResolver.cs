// SPDX-License-Identifier: Apache-2.0
using System.Runtime.CompilerServices;
using Microsoft.Extensions.Options;
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Enums;
using PepeAudio.Core.Exceptions;
using PepeAudio.Sources.Cache;
using PepeAudio.Sources.Matching;
using PepeAudio.Sources.Metadata;
using PepeAudio.Sources.Models;

namespace PepeAudio.Sources.Providers;

// Apple Music links: metadata only (DRM), matched to YouTube via the scorer + cache.
public sealed class AppleMusicResolver : ISourceResolver
{
    private readonly AppleMusicMetadataClient _apple;
    private readonly AppleMusicPublicClient _applePublic;
    private readonly IYouTubeMatcher _matcher;
    private readonly ITrackCache _cache;
    private readonly AppleMusicOptions _opt;

    public AppleMusicResolver(AppleMusicMetadataClient apple, AppleMusicPublicClient applePublic,
        IYouTubeMatcher matcher, ITrackCache cache, IOptions<AppleMusicOptions> opt)
    {
        _apple = apple;
        _applePublic = applePublic;
        _matcher = matcher;
        _cache = cache;
        _opt = opt.Value;
    }

    public SourceKind Kind => SourceKind.AppleMusic;
    public int Priority => 25;

    public bool CanHandle(ResolveRequest req)
        => req.Attachment is null && req.HasUrl && AppleMusicUrl.IsAppleMusic(req.Url);

    public async IAsyncEnumerable<PlayableRef> ResolveAsync(
        ResolveRequest req, [EnumeratorCancellation] CancellationToken ct)
    {
        if (!AppleMusicUrl.TryParse(req.Url, _opt.Storefront, out var reference))
            throw new ResolveFailedException("認識できない Apple Music の URL です。");

        // Fall back to the credential-free iTunes Lookup API when no developer token is set.
        var queries = _apple.Enabled
            ? await _apple.ResolveAsync(reference, ct)
            : await _applePublic.ResolveAsync(reference, ct);

        await foreach (var track in MetadataMatch.MatchAndCacheAsync(
            queries, "apple", _matcher, _cache, req.RequesterId, ct))
            yield return track;
    }
}
