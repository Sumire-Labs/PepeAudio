// SPDX-License-Identifier: Apache-2.0
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Routing;
using PepeAudio.Application.Playback;
using PepeAudio.Core.Contracts;
using PepeAudio.Data.Repositories;

namespace PepeAudio.Web.Api;

// Per-user dashboard playlists. Ownership is enforced in the repository (every query is
// scoped by user_id); loading a playlist into a guild queue goes through the hub, not here.
public static class PlaylistsEndpoints
{
    private const int ImportMax = PlaylistRepository.MaxTracksPerPlaylist;

    public static void MapPlaylistsEndpoints(this IEndpointRouteBuilder app)
    {
        var group = app.MapGroup("/api/playlists").RequireAuthorization();

        group.MapGet("", async (HttpContext ctx, IPlaylistRepository repo, CancellationToken ct) =>
        {
            if (!TryUser(ctx, out var uid)) return Results.Unauthorized();
            return Results.Ok(new { playlists = await repo.ListAsync(uid, ct) });
        });

        group.MapPost("", async (HttpContext ctx, CreateBody body, IPlaylistRepository repo, CancellationToken ct) =>
        {
            if (!TryUser(ctx, out var uid)) return Results.Unauthorized();
            var (playlist, error) = await repo.CreateAsync(uid, body?.Name, ct);
            return error is not null ? Results.BadRequest(new { error }) : Results.Ok(new { playlist });
        });

        group.MapGet("/{id}", async (HttpContext ctx, string id, IPlaylistRepository repo, CancellationToken ct) =>
        {
            if (!TryUser(ctx, out var uid)) return Results.Unauthorized();
            if (!Guid.TryParse(id, out var pid)) return Results.NotFound(new { error = "not_found" });
            var detail = await repo.GetAsync(uid, pid, ct);
            return detail is null ? Results.NotFound(new { error = "not_found" }) : Results.Ok(new { playlist = detail });
        });

        group.MapPatch("/{id}", async (HttpContext ctx, string id, PatchBody body, IPlaylistRepository repo, CancellationToken ct) =>
        {
            if (!TryUser(ctx, out var uid)) return Results.Unauthorized();
            if (!Guid.TryParse(id, out var pid)) return Results.NotFound(new { error = "not_found" });
            if (body is null) return Results.BadRequest(new { error = "bad_request" });
            if (body.Name is not null && !await repo.RenameAsync(uid, pid, body.Name, ct))
                return Results.BadRequest(new { error = "名前を変更できませんでした。" });
            if (body.Tracks is not null)
            {
                var error = await repo.ReplaceTracksAsync(uid, pid, body.Tracks, ct);
                if (error is not null) return Results.BadRequest(new { error });
            }
            var detail = await repo.GetAsync(uid, pid, ct);
            return detail is null ? Results.NotFound(new { error = "not_found" }) : Results.Ok(new { playlist = detail });
        });

        group.MapDelete("/{id}", async (HttpContext ctx, string id, IPlaylistRepository repo, CancellationToken ct) =>
        {
            if (!TryUser(ctx, out var uid)) return Results.Unauthorized();
            if (!Guid.TryParse(id, out var pid)) return Results.NotFound(new { error = "not_found" });
            return await repo.DeleteAsync(uid, pid, ct) ? Results.Ok(new { ok = true }) : Results.NotFound(new { error = "not_found" });
        });

        group.MapPost("/{id}/tracks", async (HttpContext ctx, string id, AddTrackBody body, IPlaylistRepository repo, CancellationToken ct) =>
        {
            if (!TryUser(ctx, out var uid)) return Results.Unauthorized();
            if (!Guid.TryParse(id, out var pid)) return Results.NotFound(new { error = "not_found" });
            if (body?.Track is null) return Results.BadRequest(new { error = "不正なトラックです。" });
            var error = await repo.AddTrackAsync(uid, pid, body.Track, ct);
            if (error is not null) return Results.BadRequest(new { error });
            var detail = await repo.GetAsync(uid, pid, ct);
            return Results.Ok(new { playlist = detail });
        });

        // Resolve a URL into per-track metadata (the resolver's SSRF guards apply) and append.
        group.MapPost("/{id}/import", async (HttpContext ctx, string id, ImportBody body,
            IPlaylistRepository repo, IPlaybackService playback, CancellationToken ct) =>
        {
            if (!TryUser(ctx, out var uid)) return Results.Unauthorized();
            if (!Guid.TryParse(id, out var pid)) return Results.NotFound(new { error = "not_found" });
            var url = body?.Url?.Trim();
            if (string.IsNullOrEmpty(url) || url.Length > 2000) return Results.BadRequest(new { error = "URL を入力してください。" });
            if (await repo.GetAsync(uid, pid, ct) is null) return Results.NotFound(new { error = "not_found" });

            IReadOnlyList<TrackInfo> resolved;
            try { resolved = await playback.ResolveInfoAsync(url, uid, ImportMax, ct); }
            catch { return Results.BadRequest(new { error = "読み込みに失敗しました。" }); }
            if (resolved.Count == 0) return Results.BadRequest(new { error = "曲が見つかりませんでした。" });

            var tracks = resolved.Select(t => new PlaylistTrack(
                t.Url, t.Title, t.Artist, t.ThumbnailUrl, (int)t.Source, t.DurationMs > 0 ? t.DurationMs : null)).ToList();
            var (added, error) = await repo.AddTracksAsync(uid, pid, tracks, ct);
            if (error is not null) return Results.BadRequest(new { error });
            var detail = await repo.GetAsync(uid, pid, ct);
            return Results.Ok(new { playlist = detail, added });
        });
    }

    private static bool TryUser(HttpContext ctx, out ulong id)
        => ulong.TryParse(ctx.User.FindFirst("sub")?.Value, out id);

    private sealed record CreateBody(string? Name);
    private sealed record PatchBody(string? Name, PlaylistTrack[]? Tracks);
    private sealed record AddTrackBody(PlaylistTrack? Track);
    private sealed record ImportBody(string? Url);
}
