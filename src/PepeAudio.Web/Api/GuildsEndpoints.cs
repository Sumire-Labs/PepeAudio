// SPDX-License-Identifier: Apache-2.0
using Discord.WebSocket;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Routing;
using Microsoft.Extensions.Caching.Memory;
using PepeAudio.Application.Playback;
using PepeAudio.Web.Auth;

namespace PepeAudio.Web.Api;

// The manageable-guild list = (user's manage-able guilds) intersect (guilds the bot is in).
public static class GuildsEndpoints
{
    public static void MapGuildsEndpoints(this IEndpointRouteBuilder app)
    {
        app.MapGet("/api/guilds", (HttpContext ctx, IMemoryCache cache, DiscordShardedClient client) =>
        {
            var userGuilds = GuildAccess.ManageableGuilds(ctx.User, cache);
            var botGuilds = client.Guilds.Select(g => g.Id.ToString()).ToHashSet();
            var result = userGuilds
                .Where(g => botGuilds.Contains(g.Id))
                .Select(g => new { g.Id, g.Name, g.Icon, g.Owner });
            return Results.Ok(result);
        }).RequireAuthorization();

        app.MapGet("/api/guilds/{guildId}/player", (HttpContext ctx, string guildId,
            IMemoryCache cache, IPlaybackService playback) =>
        {
            if (!GuildAccess.CanManage(ctx.User, cache, guildId) || !ulong.TryParse(guildId, out var id))
                return Results.Forbid();
            return Results.Ok(playback.GetState(id));
        }).RequireAuthorization();
    }
}
