// SPDX-License-Identifier: Apache-2.0
using System.Security.Cryptography;
using System.Text;
using PepeAudio.Core.Contracts;

namespace PepeAudio.Sources.Cache;

// Caches the resolution of a source reference (e.g. a Spotify id or ISRC) to a
// playable track, so repeated links skip the API + search. Never caches the
// short-lived stream URL (that stays JIT via IStreamProvider).
public interface ITrackCache
{
    Task<PlayableRef?> GetAsync(string cacheKey, ulong requesterId, CancellationToken ct);
    Task SetAsync(string cacheKey, PlayableRef track, CancellationToken ct);
}

public static class CacheKeys
{
    public static string For(string source, string id) => Hash($"{source}:{id}");
    public static string ForIsrc(string isrc) => Hash($"isrc:{isrc}");

    private static string Hash(string s)
        => Convert.ToHexString(SHA1.HashData(Encoding.UTF8.GetBytes(s))).ToLowerInvariant();
}
