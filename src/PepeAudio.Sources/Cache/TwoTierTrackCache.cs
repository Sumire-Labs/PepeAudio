// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core.Contracts;

namespace PepeAudio.Sources.Cache;

// Read-through: Valkey (hot) then PostgreSQL (durable). Writes populate both.
public sealed class TwoTierTrackCache : ITrackCache
{
    private readonly ValkeyTrackCache _hot;
    private readonly PostgresTrackCacheStore _durable;

    public TwoTierTrackCache(ValkeyTrackCache hot, PostgresTrackCacheStore durable)
    {
        _hot = hot;
        _durable = durable;
    }

    public async Task<PlayableRef?> GetAsync(string cacheKey, ulong requesterId, CancellationToken ct)
    {
        var hit = await _hot.GetAsync(cacheKey, requesterId, ct);
        if (hit is not null) return hit;

        hit = await _durable.GetAsync(cacheKey, requesterId, ct);
        if (hit is not null) await _hot.SetAsync(cacheKey, hit, ct);
        return hit;
    }

    public async Task SetAsync(string cacheKey, PlayableRef track, CancellationToken ct)
    {
        await _hot.SetAsync(cacheKey, track, ct);
        await _durable.SetAsync(cacheKey, track, ct);
    }
}
