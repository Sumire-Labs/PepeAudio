// SPDX-License-Identifier: Apache-2.0
using System.Net.Http.Headers;
using System.Text.Json;
using Microsoft.Extensions.Options;
using PepeAudio.Core.Contracts;

namespace PepeAudio.Sources.Metadata;

// Apple Music catalog metadata (developer token). Audio is DRM: metadata only.
public sealed class AppleMusicMetadataClient
{
    private static readonly TimeSpan TokenLifetime = TimeSpan.FromDays(150);

    private readonly IHttpClientFactory _httpFactory;
    private readonly AppleMusicOptions _opt;
    private readonly SemaphoreSlim _tokenLock = new(1, 1);
    private string? _token;
    private DateTimeOffset _expiry;

    public AppleMusicMetadataClient(IHttpClientFactory httpFactory, IOptions<AppleMusicOptions> opt)
    {
        _httpFactory = httpFactory;
        _opt = opt.Value;
    }

    public bool Enabled => _opt.Enabled;

    public async Task<IReadOnlyList<MatchQuery>> ResolveAsync(AppleRef reference, CancellationToken ct)
    {
        var token = await TokenAsync(ct);
        var path = reference.Kind switch
        {
            AppleKind.Album => $"albums/{reference.Id}/tracks",
            AppleKind.Playlist => $"playlists/{reference.Id}/tracks",
            _ => $"songs/{reference.Id}",
        };

        using var req = new HttpRequestMessage(HttpMethod.Get,
            $"https://api.music.apple.com/v1/catalog/{reference.Storefront}/{path}");
        req.Headers.Authorization = new AuthenticationHeaderValue("Bearer", token);
        using var http = _httpFactory.CreateClient();
        using var resp = await http.SendAsync(req, ct);
        resp.EnsureSuccessStatusCode();

        using var doc = await JsonDocument.ParseAsync(await resp.Content.ReadAsStreamAsync(ct), default, ct);
        var list = new List<MatchQuery>();
        foreach (var item in doc.RootElement.GetProperty("data").EnumerateArray())
            if (item.TryGetProperty("attributes", out var attr))
                list.Add(ToQuery(attr));
        return list;
    }

    private static MatchQuery ToQuery(JsonElement attr)
    {
        string? S(string name) => attr.TryGetProperty(name, out var v) ? v.GetString() : null;
        var duration = attr.TryGetProperty("durationInMillis", out var d) ? d.GetInt64() : 0;
        return new MatchQuery(S("name") ?? "", S("artistName") ?? "", duration, S("isrc"));
    }

    private async Task<string> TokenAsync(CancellationToken ct)
    {
        if (_token is not null && DateTimeOffset.UtcNow < _expiry) return _token;
        await _tokenLock.WaitAsync(ct);
        try
        {
            if (_token is not null && DateTimeOffset.UtcNow < _expiry) return _token;
            var pem = await File.ReadAllTextAsync(_opt.PrivateKeyPath!, ct);
            var now = DateTimeOffset.UtcNow;
            _token = AppleDeveloperToken.Create(_opt.TeamId!, _opt.KeyId!, pem, now, TokenLifetime);
            _expiry = now.Add(TokenLifetime).AddDays(-1);
            return _token;
        }
        finally { _tokenLock.Release(); }
    }
}
