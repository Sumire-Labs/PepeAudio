// SPDX-License-Identifier: Apache-2.0
using System.Net.Http.Headers;
using System.Net.Http.Json;
using System.Text.Json;
using Microsoft.Extensions.Options;

namespace PepeAudio.Web.Auth;

public sealed record DiscordUser(string Id, string Username, string? GlobalName, string? Avatar);

public sealed record UserGuild(string Id, string Name, string? Icon, bool Owner);

// Backend-authority OAuth2: the Discord client secret stays server-side.
public sealed class DiscordApiClient
{
    private const ulong ManageGuild = 0x20; // MANAGE_GUILD = 1 << 5
    // Pin the REST calls to the same API version the gateway uses; the unversioned route defaults
    // to an older version where guild `permissions` is a JSON number, not the v8+ string.
    private const string ApiBase = "https://discord.com/api/v10";

    private readonly IHttpClientFactory _httpFactory;
    private readonly WebOptions _opt;

    public DiscordApiClient(IHttpClientFactory httpFactory, IOptions<WebOptions> opt)
    {
        _httpFactory = httpFactory;
        _opt = opt.Value;
    }

    public async Task<string?> ExchangeCodeAsync(string code, CancellationToken ct)
    {
        using var http = _httpFactory.CreateClient();
        var form = new FormUrlEncodedContent(new Dictionary<string, string>
        {
            ["client_id"] = _opt.OAuth.ClientId,
            ["client_secret"] = _opt.OAuth.ClientSecret,
            ["grant_type"] = "authorization_code",
            ["code"] = code,
            ["redirect_uri"] = _opt.OAuth.RedirectUri,
        });
        using var resp = await http.PostAsync($"{ApiBase}/oauth2/token", form, ct);
        if (!resp.IsSuccessStatusCode) return null;
        var body = await resp.Content.ReadFromJsonAsync<TokenResponse>(ct);
        return body?.access_token;
    }

    public async Task<DiscordUser?> GetUserAsync(string accessToken, CancellationToken ct)
    {
        var me = await GetAsync<MeResponse>($"{ApiBase}/users/@me", accessToken, ct);
        return me is null ? null : new DiscordUser(me.id, me.username, me.global_name, me.avatar);
    }

    public async Task<IReadOnlyList<UserGuild>> GetManageableGuildsAsync(string accessToken, CancellationToken ct)
    {
        var guilds = await GetAsync<GuildResponse[]>($"{ApiBase}/users/@me/guilds", accessToken, ct);
        return (guilds ?? Array.Empty<GuildResponse>())
            .Where(g => g.owner || (Permissions(g.permissions) & ManageGuild) != 0)
            .Select(g => new UserGuild(g.id, g.name, g.icon, g.owner))
            .ToList();
    }

    // `permissions` is a string on API v8+ but a number on older/unversioned routes — accept both so
    // a version drift can't throw and 500 the OAuth callback.
    private static ulong Permissions(JsonElement p) => p.ValueKind switch
    {
        JsonValueKind.String => ulong.TryParse(p.GetString(), out var s) ? s : 0,
        JsonValueKind.Number => p.TryGetUInt64(out var n) ? n : 0,
        _ => 0,
    };

    private async Task<T?> GetAsync<T>(string url, string accessToken, CancellationToken ct)
    {
        using var http = _httpFactory.CreateClient();
        using var req = new HttpRequestMessage(HttpMethod.Get, url);
        req.Headers.Authorization = new AuthenticationHeaderValue("Bearer", accessToken);
        using var resp = await http.SendAsync(req, ct);
        return resp.IsSuccessStatusCode ? await resp.Content.ReadFromJsonAsync<T>(ct) : default;
    }

    private sealed record TokenResponse(string access_token);
    private sealed record MeResponse(string id, string username, string? global_name, string? avatar);
    private sealed record GuildResponse(string id, string name, string? icon, bool owner, JsonElement permissions);
}
