// SPDX-License-Identifier: Apache-2.0
using System.Text.Json;
using Microsoft.Extensions.Logging;
using PepeAudio.Cache;
using PepeAudio.Core.Contracts;

namespace PepeAudio.Sources.Cache;

// Hot cache in Valkey (short TTL). Best-effort: unavailable Valkey is a no-op.
public sealed class ValkeyTrackCache : ITrackCache
{
    private static readonly TimeSpan Ttl = TimeSpan.FromHours(6);

    private readonly IValkeyConnection _valkey;
    private readonly ILogger<ValkeyTrackCache> _log;

    public ValkeyTrackCache(IValkeyConnection valkey, ILogger<ValkeyTrackCache> log)
    {
        _valkey = valkey;
        _log = log;
    }

    public async Task<PlayableRef?> GetAsync(string cacheKey, ulong requesterId, CancellationToken ct)
    {
        var db = _valkey.TryGetDatabase();
        if (db is null) return null;
        try
        {
            var val = await db.StringGetAsync(ValkeyKeys.Track(cacheKey));
            if (!val.HasValue) return null;
            var track = JsonSerializer.Deserialize<PlayableRef>((string)val!);
            return track is null ? null : track with { Info = track.Info with { RequestedBy = requesterId } };
        }
        catch (Exception ex) { _log.LogDebug(ex, "Track cache read skipped"); return null; }
    }

    public async Task SetAsync(string cacheKey, PlayableRef track, CancellationToken ct)
    {
        var db = _valkey.TryGetDatabase();
        if (db is null) return;
        try { await db.StringSetAsync(ValkeyKeys.Track(cacheKey), JsonSerializer.Serialize(track), Ttl); }
        catch (Exception ex) { _log.LogDebug(ex, "Track cache write skipped"); }
    }
}
