// SPDX-License-Identifier: Apache-2.0
using Dapper;
using Microsoft.Extensions.Logging;
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Enums;
using PepeAudio.Data;

namespace PepeAudio.Sources.Cache;

// Durable cache in PostgreSQL. Stores the resolved track (never the stream URL).
public sealed class PostgresTrackCacheStore : ITrackCache
{
    private readonly INpgsqlDataSourceProvider _db;
    private readonly ILogger<PostgresTrackCacheStore> _log;

    public PostgresTrackCacheStore(INpgsqlDataSourceProvider db, ILogger<PostgresTrackCacheStore> log)
    {
        _db = db;
        _log = log;
    }

    public async Task<PlayableRef?> GetAsync(string cacheKey, ulong requesterId, CancellationToken ct)
    {
        var src = _db.DataSource;
        if (src is null) return null;
        try
        {
            await using var conn = await src.OpenConnectionAsync(ct);
            var row = await conn.QuerySingleOrDefaultAsync<Row>(
                "SELECT source_id, title, author, duration_ms, thumbnail_url FROM track_cache WHERE cache_key = @k",
                new { k = cacheKey });
            if (row is null) return null;
            var info = new TrackInfo(row.title ?? "Unknown", row.author ?? "Unknown", SourceKind.YouTube,
                row.source_id, row.duration_ms, row.thumbnail_url, false, requesterId);
            return new PlayableRef(SourceKind.YouTube, row.source_id, true, info, NeedsResolution: true);
        }
        catch (Exception ex) { _log.LogDebug(ex, "Durable track cache read skipped"); return null; }
    }

    public async Task SetAsync(string cacheKey, PlayableRef track, CancellationToken ct)
    {
        var src = _db.DataSource;
        if (src is null) return;
        try
        {
            await using var conn = await src.OpenConnectionAsync(ct);
            await conn.ExecuteAsync(
                "INSERT INTO track_cache(cache_key, source, source_id, title, author, duration_ms, thumbnail_url) " +
                "VALUES (@k, 'youtube', @sid, @title, @author, @dur, @thumb) " +
                "ON CONFLICT (cache_key) DO UPDATE SET source_id=@sid, title=@title, author=@author, " +
                "duration_ms=@dur, thumbnail_url=@thumb, refreshed_at=now()",
                new
                {
                    k = cacheKey,
                    sid = track.Input,
                    title = track.Info.Title,
                    author = track.Info.Artist,
                    dur = (int)track.Info.DurationMs,
                    thumb = track.Info.ThumbnailUrl,
                });
        }
        catch (Exception ex) { _log.LogDebug(ex, "Durable track cache write skipped"); }
    }

    private sealed class Row
    {
        public string source_id { get; set; } = "";
        public string? title { get; set; }
        public string? author { get; set; }
        public int duration_ms { get; set; }
        public string? thumbnail_url { get; set; }
    }
}
