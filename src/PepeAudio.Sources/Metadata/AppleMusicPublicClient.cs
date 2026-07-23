// SPDX-License-Identifier: Apache-2.0
using System.Text.Json;
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Exceptions;

namespace PepeAudio.Sources.Metadata;

// Credential-free Apple Music metadata via the public iTunes Lookup API (no developer
// token). The request URL is built from a validated numeric id, never the raw pasted
// link (SSRF-safe). Songs and albums are supported; curated playlists need the API.
public sealed class AppleMusicPublicClient
{
    private const int MaxAlbumTracks = 100;
    private readonly IHttpClientFactory _httpFactory;

    public AppleMusicPublicClient(IHttpClientFactory httpFactory) => _httpFactory = httpFactory;

    public async Task<IReadOnlyList<MatchQuery>> ResolveAsync(AppleRef reference, CancellationToken ct)
    {
        if (reference.Kind == AppleKind.Playlist)
            throw new ResolveFailedException("Apple Music のプレイリストは認証情報の設定が必要です（曲・アルバムのリンクは設定不要です）。");

        var url = reference.Kind == AppleKind.Album
            ? $"https://itunes.apple.com/lookup?id={Uri.EscapeDataString(reference.Id)}&entity=song&limit={MaxAlbumTracks}"
            : $"https://itunes.apple.com/lookup?id={Uri.EscapeDataString(reference.Id)}";

        using var req = new HttpRequestMessage(HttpMethod.Get, url);
        req.Headers.TryAddWithoutValidation("User-Agent", "Mozilla/5.0");
        using var http = _httpFactory.CreateClient();
        using var resp = await http.SendAsync(req, ct);
        resp.EnsureSuccessStatusCode();
        using var doc = await JsonDocument.ParseAsync(await resp.Content.ReadAsStreamAsync(ct), default, ct);

        var list = new List<MatchQuery>();
        if (!doc.RootElement.TryGetProperty("results", out var results)) return list;
        foreach (var item in results.EnumerateArray())
        {
            // Album lookups also return the collection wrapper; keep only tracks.
            if (item.TryGetProperty("wrapperType", out var w) && w.GetString() != "track") continue;
            var name = Str(item, "trackName");
            if (name is null) continue;
            var dur = item.TryGetProperty("trackTimeMillis", out var d) && d.ValueKind == JsonValueKind.Number ? d.GetInt64() : 0;
            list.Add(new MatchQuery(name, Str(item, "artistName") ?? "", dur, null));
        }
        return list;
    }

    private static string? Str(JsonElement e, string name)
        => e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;
}
