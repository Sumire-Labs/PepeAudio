// SPDX-License-Identifier: Apache-2.0
using System.Net.Http.Headers;
using System.Net.Http.Json;
using System.Text;
using System.Text.Json;
using Microsoft.Extensions.Options;
using PepeAudio.Core.Contracts;

namespace PepeAudio.Sources.Metadata;

// Spotify Web API metadata (client-credentials). Audio is DRM: metadata only.
public sealed class SpotifyMetadataClient
{
    private readonly IHttpClientFactory _httpFactory;
    private readonly SpotifyOptions _opt;
    private readonly SemaphoreSlim _tokenLock = new(1, 1);
    private string? _token;
    private DateTimeOffset _expiry;

    public SpotifyMetadataClient(IHttpClientFactory httpFactory, IOptions<SpotifyOptions> opt)
    {
        _httpFactory = httpFactory;
        _opt = opt.Value;
    }

    public bool Enabled => _opt.Enabled;

    public async Task<IReadOnlyList<MatchQuery>> ResolveAsync(SpotifyRef reference, CancellationToken ct)
    {
        await AuthorizeAsync(ct);
        return reference.Kind switch
        {
            SpotifyKind.Track => new[] { ToQuery((await GetAsync($"tracks/{reference.Id}", ct)).RootElement) },
            SpotifyKind.Album => await PageAsync($"albums/{reference.Id}/tracks?limit=50", ct),
            SpotifyKind.Playlist => await PageAsync($"playlists/{reference.Id}/tracks?limit=100", ct),
            _ => Array.Empty<MatchQuery>(),
        };
    }

    private async Task<IReadOnlyList<MatchQuery>> PageAsync(string path, CancellationToken ct)
    {
        using var doc = await GetAsync(path, ct);
        var items = doc.RootElement.GetProperty("items");
        var list = new List<MatchQuery>();
        foreach (var item in items.EnumerateArray())
        {
            var track = item.TryGetProperty("track", out var t) ? t : item;
            if (track.ValueKind == JsonValueKind.Object && track.TryGetProperty("name", out _))
                list.Add(ToQuery(track));
        }
        return list;
    }

    private static MatchQuery ToQuery(JsonElement track)
    {
        var title = track.GetProperty("name").GetString() ?? "";
        var artist = track.TryGetProperty("artists", out var artists) && artists.GetArrayLength() > 0
            ? artists[0].GetProperty("name").GetString() ?? "" : "";
        var duration = track.TryGetProperty("duration_ms", out var d) ? d.GetInt64() : 0;
        var isrc = track.TryGetProperty("external_ids", out var ext) && ext.TryGetProperty("isrc", out var i)
            ? i.GetString() : null;
        return new MatchQuery(title, artist, duration, isrc);
    }

    private async Task<JsonDocument> GetAsync(string path, CancellationToken ct)
    {
        using var req = new HttpRequestMessage(HttpMethod.Get, $"https://api.spotify.com/v1/{path}");
        req.Headers.Authorization = new AuthenticationHeaderValue("Bearer", _token);
        using var http = _httpFactory.CreateClient();
        using var resp = await http.SendAsync(req, ct);
        resp.EnsureSuccessStatusCode();
        return await JsonDocument.ParseAsync(await resp.Content.ReadAsStreamAsync(ct), default, ct);
    }

    private async Task AuthorizeAsync(CancellationToken ct)
    {
        if (_token is not null && DateTimeOffset.UtcNow < _expiry) return;
        await _tokenLock.WaitAsync(ct);
        try
        {
            if (_token is not null && DateTimeOffset.UtcNow < _expiry) return;
            var basic = Convert.ToBase64String(Encoding.UTF8.GetBytes($"{_opt.ClientId}:{_opt.ClientSecret}"));
            using var req = new HttpRequestMessage(HttpMethod.Post, "https://accounts.spotify.com/api/token")
            {
                Content = new FormUrlEncodedContent(new[] { new KeyValuePair<string, string>("grant_type", "client_credentials") }),
            };
            req.Headers.Authorization = new AuthenticationHeaderValue("Basic", basic);
            using var http = _httpFactory.CreateClient();
            using var resp = await http.SendAsync(req, ct);
            resp.EnsureSuccessStatusCode();
            var body = await resp.Content.ReadFromJsonAsync<TokenResponse>(ct);
            _token = body!.access_token;
            _expiry = DateTimeOffset.UtcNow.AddSeconds(body.expires_in - 30);
        }
        finally { _tokenLock.Release(); }
    }

    private sealed record TokenResponse(string access_token, int expires_in);
}
