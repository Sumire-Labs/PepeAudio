// SPDX-License-Identifier: Apache-2.0
using Discord.WebSocket;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Routing;
using Microsoft.Extensions.Options;
using PepeAudio.Application.Playback;
using PepeAudio.Audio;

namespace PepeAudio.Web.Api;

// System-wide view for operators. Gated by the WebGui:AdminUserIds allowlist.
public static class AdminEndpoints
{
    public static void MapAdminEndpoints(this IEndpointRouteBuilder app)
    {
        app.MapGet("/api/admin/overview", (HttpContext ctx, IOptions<WebOptions> opt,
            IPlayerManager players, IPlaybackService playback, DiscordShardedClient client) =>
        {
            if (!IsAdmin(ctx, opt.Value))
                return Results.Forbid();

            var active = players.ActiveGuildIds.Select(id =>
            {
                var state = playback.GetState(id);
                return new
                {
                    id = id.ToString(),
                    name = client.GetGuild(id)?.Name ?? "unknown",
                    playing = state.IsPlaying,
                    current = state.Current?.Title,
                    queue = state.Queue.Count,
                };
            });

            return Results.Ok(new
            {
                botGuilds = client.Guilds.Count,
                activeVoices = players.ActiveCount,
                shards = client.Shards.Count,
                players = active,
            });
        }).RequireAuthorization();
    }

    private static bool IsAdmin(HttpContext ctx, WebOptions opt)
    {
        var id = ctx.User.FindFirst("sub")?.Value;
        return id is not null && opt.AdminUserIds.Contains(id);
    }
}
