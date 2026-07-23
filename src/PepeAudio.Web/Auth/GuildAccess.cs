// SPDX-License-Identifier: Apache-2.0
using System.Security.Claims;
using Microsoft.Extensions.Caching.Memory;

namespace PepeAudio.Web.Auth;

// One place for the per-user manageable-guild cache: shared key convention and lookup,
// used by the OAuth callback (write), the guilds endpoint and the player hub (read).
internal static class GuildAccess
{
    public static string CacheKey(string userId) => $"guilds:{userId}";

    public static IReadOnlyList<UserGuild> ManageableGuilds(ClaimsPrincipal? user, IMemoryCache cache)
    {
        var userId = user?.FindFirst("sub")?.Value;
        return userId is not null
            && cache.TryGetValue(CacheKey(userId), out IReadOnlyList<UserGuild>? g) && g is not null
            ? g : Array.Empty<UserGuild>();
    }

    // True when the user's manageable-guild list is present in cache. It's populated at login
    // and lost on a backend restart or after the sliding TTL — a valid JWT can outlive it, which
    // is exactly the "logged in but no servers" state that should trigger a silent re-login.
    public static bool HasGuildCache(ClaimsPrincipal? user, IMemoryCache cache)
    {
        var userId = user?.FindFirst("sub")?.Value;
        return userId is not null && cache.TryGetValue(CacheKey(userId), out _);
    }

    public static bool CanManage(ClaimsPrincipal? user, IMemoryCache cache, string guildId)
        => ManageableGuilds(user, cache).Any(g => g.Id == guildId);
}
