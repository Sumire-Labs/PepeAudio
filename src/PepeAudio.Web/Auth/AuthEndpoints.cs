// SPDX-License-Identifier: Apache-2.0
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Routing;
using Microsoft.Extensions.Caching.Memory;
using Microsoft.Extensions.Options;

namespace PepeAudio.Web.Auth;

// Backend-authority Discord OAuth2 endpoints. The client secret never leaves here.
public static class AuthEndpoints
{
    private const string StateCookie = "pepe_oauth_state";

    public static void MapAuthEndpoints(this IEndpointRouteBuilder app)
    {
        app.MapGet("/api/auth/login", (HttpContext ctx, IOptions<WebOptions> opt) =>
        {
            var o = opt.Value;
            var state = Guid.NewGuid().ToString("N");
            ctx.Response.Cookies.Append(StateCookie, state, Short(ctx));
            var url = "https://discord.com/oauth2/authorize" +
                $"?client_id={o.OAuth.ClientId}&response_type=code&scope=identify%20guilds" +
                $"&redirect_uri={Uri.EscapeDataString(o.OAuth.RedirectUri)}&state={state}";
            return Results.Redirect(url);
        });

        app.MapGet("/api/auth/callback", async (HttpContext ctx, string? code, string? state,
            DiscordApiClient api, JwtIssuer issuer, IMemoryCache cache, IOptions<WebOptions> opt, CancellationToken ct) =>
        {
            var o = opt.Value;
            if (code is null || state is null || ctx.Request.Cookies[StateCookie] != state)
                return Results.Redirect($"{o.BaseUrl}?error=state");
            ctx.Response.Cookies.Delete(StateCookie);

            var access = await api.ExchangeCodeAsync(code, ct);
            var user = access is null ? null : await api.GetUserAsync(access, ct);
            if (access is null || user is null)
                return Results.Redirect($"{o.BaseUrl}?error=oauth");

            var guilds = await api.GetManageableGuildsAsync(access, ct);
            // Sliding so an active session keeps authorization (a fixed 10-min TTL revoked
            // access mid-session); absolute bound caps how stale membership can get.
            cache.Set(GuildAccess.CacheKey(user.Id), guilds, new MemoryCacheEntryOptions
            {
                SlidingExpiration = TimeSpan.FromHours(1),
                AbsoluteExpirationRelativeToNow = TimeSpan.FromDays(7),
            });

            var jwt = issuer.Issue(user.Id, user.GlobalName ?? user.Username, user.Avatar);
            ctx.Response.Cookies.Append(o.SessionCookieName, jwt, Session(ctx));
            return Results.Redirect(o.BaseUrl);
        });

        app.MapGet("/api/auth/me", (HttpContext ctx) => Results.Ok(new
        {
            id = ctx.User.FindFirst("sub")?.Value,
            username = ctx.User.FindFirst("name")?.Value,
            avatar = ctx.User.FindFirst("avatar")?.Value,
        })).RequireAuthorization();

        app.MapPost("/api/auth/logout", (HttpContext ctx, IMemoryCache cache, IOptions<WebOptions> opt) =>
        {
            var id = ctx.User.FindFirst("sub")?.Value;
            if (id is not null) cache.Remove(GuildAccess.CacheKey(id));
            ctx.Response.Cookies.Delete(opt.Value.SessionCookieName);
            return Results.Ok();
        }).RequireAuthorization();
    }

    private static CookieOptions Short(HttpContext ctx) => new()
    {
        HttpOnly = true, Secure = ctx.Request.IsHttps, SameSite = SameSiteMode.Lax,
        Path = "/", MaxAge = TimeSpan.FromMinutes(10),
    };

    private static CookieOptions Session(HttpContext ctx) => new()
    {
        HttpOnly = true, Secure = ctx.Request.IsHttps, SameSite = SameSiteMode.Lax,
        Path = "/", Expires = DateTimeOffset.UtcNow.AddDays(7),
    };
}
