// SPDX-License-Identifier: Apache-2.0
using Discord.WebSocket;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Routing;
using Microsoft.Extensions.Caching.Memory;
using PepeAudio.Application.Playback;
using PepeAudio.Web.Auth;
using PepeAudio.Web.Realtime;

namespace PepeAudio.Web.Api;

// The manageable-guild list = (user's manage-able guilds) intersect (guilds the bot is in).
public static class GuildsEndpoints
{
    public static void MapGuildsEndpoints(this IEndpointRouteBuilder app)
    {
        app.MapGet("/api/guilds", (HttpContext ctx, IMemoryCache cache, DiscordShardedClient client, IPlaybackService playback) =>
        {
            var userGuilds = GuildAccess.ManageableGuilds(ctx.User, cache);
            var botGuilds = client.Guilds.Select(g => g.Id.ToString()).ToHashSet();
            // Play status is read from the local player (accurate in single-process; owner-shard
            // only when sharded — the sidebar treats a non-owned guild as idle, which self-corrects
            // once its player is opened and the owner starts broadcasting).
            // Materialize eagerly with a per-guild guard so one guild's state read can never throw
            // during response serialization and blank the whole list.
            var result = userGuilds
                .Where(g => botGuilds.Contains(g.Id))
                .Select(g =>
                {
                    var status = "idle";
                    string? currentTitle = null;
                    try
                    {
                        if (ulong.TryParse(g.Id, out var gid))
                        {
                            var state = playback.GetState(gid);
                            if (state.Current is not null)
                            {
                                status = state.IsPlaying ? "playing" : "paused";
                                currentTitle = state.Current.Title;
                            }
                        }
                    }
                    catch { /* leave as idle */ }
                    return new { g.Id, g.Name, g.Icon, g.Owner, Status = status, CurrentTitle = currentTitle };
                })
                .ToList();
            return Results.Ok(result);
        }).RequireAuthorization();

        app.MapGet("/api/guilds/{guildId}/player", (HttpContext ctx, string guildId,
            IMemoryCache cache, IPlaybackService playback, DiscordShardedClient client) =>
        {
            if (!GuildAccess.CanManage(ctx.User, cache, guildId) || !ulong.TryParse(guildId, out var id))
                return Results.Forbid();
            return Results.Ok(PlayerSnapshot.From(playback.GetState(id), client, playback.PresetNames));
        }).RequireAuthorization();

        // Search (add-track panel). Query-only; no guild scope, any authenticated user.
        app.MapPost("/api/search", async (SearchBody body, IPlaybackService playback, CancellationToken ct) =>
        {
            var query = body?.Query?.Trim();
            if (string.IsNullOrEmpty(query) || query.Length > 200)
                return Results.BadRequest(new { error = "検索語を入力してください。" });
            var candidates = await playback.SearchAsync(query, ct);
            return Results.Ok(new { candidates });
        }).RequireAuthorization();
    }

    private sealed record SearchBody(string? Query);
}
