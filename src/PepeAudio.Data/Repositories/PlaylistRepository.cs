// SPDX-License-Identifier: Apache-2.0
using System.Text;
using Dapper;
using Microsoft.Extensions.Logging;
using PepeAudio.Core.Contracts;

namespace PepeAudio.Data.Repositories;

public interface IPlaylistRepository
{
    Task<IReadOnlyList<PlaylistSummary>> ListAsync(ulong userId, CancellationToken ct);
    Task<PlaylistDetail?> GetAsync(ulong userId, Guid id, CancellationToken ct);
    Task<(PlaylistSummary? Playlist, string? Error)> CreateAsync(ulong userId, string? name, CancellationToken ct);
    Task<bool> RenameAsync(ulong userId, Guid id, string? name, CancellationToken ct);
    Task<string?> ReplaceTracksAsync(ulong userId, Guid id, IReadOnlyList<PlaylistTrack> tracks, CancellationToken ct);
    Task<bool> DeleteAsync(ulong userId, Guid id, CancellationToken ct);
    Task<string?> AddTrackAsync(ulong userId, Guid id, PlaylistTrack track, CancellationToken ct);
    Task<(int Added, string? Error)> AddTracksAsync(ulong userId, Guid id, IReadOnlyList<PlaylistTrack> tracks, CancellationToken ct);
}

public sealed class PlaylistRepository : IPlaylistRepository
{
    public const int MaxPlaylistsPerUser = 25;
    public const int MaxTracksPerPlaylist = 50;
    private const int MaxNameLength = 100;
    private const int MaxFieldLength = 500;
    private const int MaxSource = 5; // SourceKind.Attachment

    private const string SummaryColumns =
        "p.id::text AS Id, p.name AS Name, " +
        "(SELECT COUNT(*) FROM web_playlist_tracks t WHERE t.playlist_id = p.id)::int AS TrackCount, " +
        "(EXTRACT(EPOCH FROM p.updated_at) * 1000)::bigint AS UpdatedAt";

    private readonly INpgsqlDataSourceProvider _db;
    private readonly ILogger<PlaylistRepository> _log;

    public PlaylistRepository(INpgsqlDataSourceProvider db, ILogger<PlaylistRepository> log)
    {
        _db = db;
        _log = log;
    }

    public async Task<IReadOnlyList<PlaylistSummary>> ListAsync(ulong userId, CancellationToken ct)
    {
        var src = _db.DataSource;
        if (src is null) return Array.Empty<PlaylistSummary>();
        try
        {
            await using var conn = await src.OpenConnectionAsync(ct);
            var rows = await conn.QueryAsync<PlaylistSummary>(new CommandDefinition(
                $"SELECT {SummaryColumns} FROM web_playlists p WHERE p.user_id = @userId ORDER BY p.updated_at DESC",
                new { userId = (long)userId }, cancellationToken: ct));
            return rows.ToList();
        }
        catch (Exception ex) when (ex is not OperationCanceledException)
        {
            _log.LogDebug(ex, "Playlist list failed for {User}", userId);
            return Array.Empty<PlaylistSummary>();
        }
    }

    public async Task<PlaylistDetail?> GetAsync(ulong userId, Guid id, CancellationToken ct)
    {
        var src = _db.DataSource;
        if (src is null) return null;
        try
        {
            await using var conn = await src.OpenConnectionAsync(ct);
            var summary = await conn.QuerySingleOrDefaultAsync<PlaylistSummary>(new CommandDefinition(
                $"SELECT {SummaryColumns} FROM web_playlists p WHERE p.id = @id AND p.user_id = @userId",
                new { id, userId = (long)userId }, cancellationToken: ct));
            if (summary is null) return null;
            var tracks = await conn.QueryAsync<PlaylistTrack>(new CommandDefinition(
                "SELECT source_url AS SourceUrl, title AS Title, artist AS Artist, thumbnail_url AS ThumbnailUrl, " +
                "source_type::int AS Source, duration_ms AS DurationMs " +
                "FROM web_playlist_tracks WHERE playlist_id = @id ORDER BY position ASC",
                new { id }, cancellationToken: ct));
            var list = tracks.ToList();
            return new PlaylistDetail(summary.Id, summary.Name, list.Count, summary.UpdatedAt, list);
        }
        catch (Exception ex) when (ex is not OperationCanceledException)
        {
            _log.LogDebug(ex, "Playlist get failed for {User}/{Id}", userId, id);
            return null;
        }
    }

    public async Task<(PlaylistSummary?, string?)> CreateAsync(ulong userId, string? name, CancellationToken ct)
    {
        var clean = SanitizeName(name);
        if (clean is null) return (null, "名前を入力してください。");
        var src = _db.DataSource;
        if (src is null) return (null, "現在保存できません。");
        try
        {
            await using var conn = await src.OpenConnectionAsync(ct);
            var count = await conn.ExecuteScalarAsync<int>(new CommandDefinition(
                "SELECT COUNT(*) FROM web_playlists WHERE user_id = @userId", new { userId = (long)userId }, cancellationToken: ct));
            if (count >= MaxPlaylistsPerUser) return (null, $"プレイリストは最大 {MaxPlaylistsPerUser} 個までです。");
            var id = Guid.NewGuid();
            await conn.ExecuteAsync(new CommandDefinition(
                "INSERT INTO web_playlists(id, user_id, name) VALUES (@id, @userId, @name)",
                new { id, userId = (long)userId, name = clean }, cancellationToken: ct));
            return (new PlaylistSummary(id.ToString(), clean, 0, DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()), null);
        }
        catch (Exception ex) when (ex is not OperationCanceledException)
        {
            _log.LogDebug(ex, "Playlist create failed for {User}", userId);
            return (null, "保存に失敗しました。");
        }
    }

    public async Task<bool> RenameAsync(ulong userId, Guid id, string? name, CancellationToken ct)
    {
        var clean = SanitizeName(name);
        if (clean is null) return false;
        var src = _db.DataSource;
        if (src is null) return false;
        try
        {
            await using var conn = await src.OpenConnectionAsync(ct);
            var rows = await conn.ExecuteAsync(new CommandDefinition(
                "UPDATE web_playlists SET name = @name, updated_at = now() WHERE id = @id AND user_id = @userId",
                new { name = clean, id, userId = (long)userId }, cancellationToken: ct));
            return rows > 0;
        }
        catch (Exception ex) when (ex is not OperationCanceledException)
        {
            _log.LogDebug(ex, "Playlist rename failed for {User}/{Id}", userId, id);
            return false;
        }
    }

    public async Task<string?> ReplaceTracksAsync(ulong userId, Guid id, IReadOnlyList<PlaylistTrack> tracks, CancellationToken ct)
    {
        var src = _db.DataSource;
        if (src is null) return "現在保存できません。";
        var clean = tracks.Where(IsValid).Select(NormalizeForSave).Take(MaxTracksPerPlaylist).ToList();
        try
        {
            await using var conn = await src.OpenConnectionAsync(ct);
            if (!await OwnsAsync(conn, userId, id, ct)) return "プレイリストが見つかりません。";
            await using var tx = await conn.BeginTransactionAsync(ct);
            await conn.ExecuteAsync(new CommandDefinition("DELETE FROM web_playlist_tracks WHERE playlist_id = @id", new { id }, tx, cancellationToken: ct));
            await InsertTracksAsync(conn, tx, id, 0, clean, ct);
            await conn.ExecuteAsync(new CommandDefinition("UPDATE web_playlists SET updated_at = now() WHERE id = @id", new { id }, tx, cancellationToken: ct));
            await tx.CommitAsync(ct);
            return null;
        }
        catch (Exception ex) when (ex is not OperationCanceledException)
        {
            _log.LogDebug(ex, "Playlist replaceTracks failed for {User}/{Id}", userId, id);
            return "保存に失敗しました。";
        }
    }

    public async Task<bool> DeleteAsync(ulong userId, Guid id, CancellationToken ct)
    {
        var src = _db.DataSource;
        if (src is null) return false;
        try
        {
            await using var conn = await src.OpenConnectionAsync(ct);
            var rows = await conn.ExecuteAsync(new CommandDefinition(
                "DELETE FROM web_playlists WHERE id = @id AND user_id = @userId", new { id, userId = (long)userId }, cancellationToken: ct));
            return rows > 0;
        }
        catch (Exception ex) when (ex is not OperationCanceledException)
        {
            _log.LogDebug(ex, "Playlist delete failed for {User}/{Id}", userId, id);
            return false;
        }
    }

    public async Task<string?> AddTrackAsync(ulong userId, Guid id, PlaylistTrack track, CancellationToken ct)
    {
        if (!IsValid(track)) return "不正なトラックです。";
        var src = _db.DataSource;
        if (src is null) return "現在保存できません。";
        try
        {
            await using var conn = await src.OpenConnectionAsync(ct);
            if (!await OwnsAsync(conn, userId, id, ct)) return "プレイリストが見つかりません。";
            var count = await conn.ExecuteScalarAsync<int>(new CommandDefinition(
                "SELECT COUNT(*) FROM web_playlist_tracks WHERE playlist_id = @id", new { id }, cancellationToken: ct));
            if (count >= MaxTracksPerPlaylist) return $"プレイリストは最大 {MaxTracksPerPlaylist} 曲までです。";
            await using var tx = await conn.BeginTransactionAsync(ct);
            await InsertTracksAsync(conn, tx, id, count, new[] { NormalizeForSave(track) }, ct);
            await conn.ExecuteAsync(new CommandDefinition("UPDATE web_playlists SET updated_at = now() WHERE id = @id", new { id }, tx, cancellationToken: ct));
            await tx.CommitAsync(ct);
            return null;
        }
        catch (Exception ex) when (ex is not OperationCanceledException)
        {
            _log.LogDebug(ex, "Playlist addTrack failed for {User}/{Id}", userId, id);
            return "保存に失敗しました。";
        }
    }

    public async Task<(int, string?)> AddTracksAsync(ulong userId, Guid id, IReadOnlyList<PlaylistTrack> tracks, CancellationToken ct)
    {
        var src = _db.DataSource;
        if (src is null) return (0, "現在保存できません。");
        try
        {
            await using var conn = await src.OpenConnectionAsync(ct);
            if (!await OwnsAsync(conn, userId, id, ct)) return (0, "プレイリストが見つかりません。");
            var count = await conn.ExecuteScalarAsync<int>(new CommandDefinition(
                "SELECT COUNT(*) FROM web_playlist_tracks WHERE playlist_id = @id", new { id }, cancellationToken: ct));
            var remaining = MaxTracksPerPlaylist - count;
            if (remaining <= 0) return (0, $"プレイリストは最大 {MaxTracksPerPlaylist} 曲までです。");
            var clean = tracks.Where(IsValid).Select(NormalizeForSave).Take(remaining).ToList();
            if (clean.Count == 0) return (0, "追加できる曲がありませんでした。");
            await using var tx = await conn.BeginTransactionAsync(ct);
            await InsertTracksAsync(conn, tx, id, count, clean, ct);
            await conn.ExecuteAsync(new CommandDefinition("UPDATE web_playlists SET updated_at = now() WHERE id = @id", new { id }, tx, cancellationToken: ct));
            await tx.CommitAsync(ct);
            return (clean.Count, null);
        }
        catch (Exception ex) when (ex is not OperationCanceledException)
        {
            _log.LogDebug(ex, "Playlist addTracks failed for {User}/{Id}", userId, id);
            return (0, "保存に失敗しました。");
        }
    }

    private static async Task<bool> OwnsAsync(System.Data.Common.DbConnection conn, ulong userId, Guid id, CancellationToken ct)
        => await conn.ExecuteScalarAsync<int>(new CommandDefinition(
            "SELECT COUNT(*) FROM web_playlists WHERE id = @id AND user_id = @userId",
            new { id, userId = (long)userId }, cancellationToken: ct)) > 0;

    private static async Task InsertTracksAsync(System.Data.Common.DbConnection conn, System.Data.Common.DbTransaction tx,
        Guid id, int basePosition, IReadOnlyList<PlaylistTrack> tracks, CancellationToken ct)
    {
        for (var i = 0; i < tracks.Count; i++)
        {
            var t = tracks[i];
            await conn.ExecuteAsync(new CommandDefinition(
                "INSERT INTO web_playlist_tracks(playlist_id, position, source_url, title, artist, thumbnail_url, source_type, duration_ms) " +
                "VALUES (@id, @pos, @url, @title, @artist, @thumb, @source, @dur)",
                new
                {
                    id, pos = basePosition + i, url = t.SourceUrl, title = t.Title, artist = t.Artist,
                    thumb = t.ThumbnailUrl, source = (short)t.Source, dur = t.DurationMs,
                }, tx, cancellationToken: ct));
        }
    }

    // Strips ASCII control chars so a name can't smuggle newlines/escapes, then trims + caps.
    private static string? SanitizeName(string? name)
    {
        if (name is null) return null;
        var sb = new StringBuilder(name.Length);
        foreach (var ch in name)
            if (ch >= 0x20 && ch != 0x7f) sb.Append(ch);
        var cleaned = sb.ToString().Trim();
        if (cleaned.Length > MaxNameLength) cleaned = cleaned[..MaxNameLength];
        return cleaned.Length > 0 ? cleaned : null;
    }

    private static bool IsValid(PlaylistTrack t) =>
        !string.IsNullOrWhiteSpace(t.SourceUrl) && t.SourceUrl.Length <= MaxFieldLength &&
        t.Title.Length <= MaxFieldLength && t.Artist.Length <= MaxFieldLength &&
        t.Source is >= 0 and <= MaxSource;

    // Collection (playlist/album/set) URLs are shared by every track in the collection, so
    // persisting one verbatim would re-import the whole collection on load. Rewrite it to an
    // "artist title" search string; idempotent for per-track URLs and existing search strings.
    private static PlaylistTrack NormalizeForSave(PlaylistTrack t)
    {
        if (!LooksLikeCollection(t.SourceUrl)) return t;
        var query = $"{t.Artist} {t.Title}".Trim();
        if (query.Length == 0) query = t.Title.Trim();
        if (query.Length > MaxFieldLength) query = query[..MaxFieldLength];
        return query.Length > 0 ? t with { SourceUrl = query } : t;
    }

    private static bool LooksLikeCollection(string url)
    {
        if (!Uri.TryCreate(url, UriKind.Absolute, out var parsed)) return false;
        var host = parsed.Host.ToLowerInvariant();
        var path = parsed.AbsolutePath.ToLowerInvariant();
        if (host is "youtube.com" or "youtu.be" || host.EndsWith(".youtube.com"))
            return path.StartsWith("/playlist") || (HasQueryKey(parsed.Query, "list") && !HasQueryKey(parsed.Query, "v"));
        if (host is "spotify.com" || host.EndsWith(".spotify.com"))
            return path.Contains("/playlist/") || path.Contains("/album/");
        if (host == "music.apple.com")
            return path.Contains("/playlist/") || (path.Contains("/album/") && !HasQueryKey(parsed.Query, "i"));
        if (host is "soundcloud.com" || host.EndsWith(".soundcloud.com"))
            return path.Contains("/sets/");
        return false;
    }

    private static bool HasQueryKey(string query, string key)
    {
        foreach (var part in query.TrimStart('?').Split('&', StringSplitOptions.RemoveEmptyEntries))
        {
            var eq = part.IndexOf('=');
            var k = eq >= 0 ? part[..eq] : part;
            if (string.Equals(k, key, StringComparison.OrdinalIgnoreCase)) return true;
        }
        return false;
    }
}
