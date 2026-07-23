// SPDX-License-Identifier: Apache-2.0
using System.Net;
using System.Text.RegularExpressions;
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Exceptions;

namespace PepeAudio.Sources.Metadata;

// Credential-free Spotify metadata by reading the public track page's Open Graph tags
// (no API key). The fetch URL is rebuilt from the validated track id (SSRF-safe). Track
// links only; albums/playlists need the API for a per-track list.
public sealed class SpotifyPublicClient
{
    private readonly IHttpClientFactory _httpFactory;

    public SpotifyPublicClient(IHttpClientFactory httpFactory) => _httpFactory = httpFactory;

    public async Task<IReadOnlyList<MatchQuery>> ResolveAsync(SpotifyRef reference, CancellationToken ct)
    {
        if (reference.Kind != SpotifyKind.Track)
            throw new ResolveFailedException("Spotify のアルバム/プレイリストは認証情報の設定が必要です（曲のリンクは設定不要です）。");

        using var req = new HttpRequestMessage(HttpMethod.Get, $"https://open.spotify.com/track/{reference.Id}");
        req.Headers.TryAddWithoutValidation("User-Agent", "Mozilla/5.0");
        using var http = _httpFactory.CreateClient();
        using var resp = await http.SendAsync(req, ct);
        resp.EnsureSuccessStatusCode();
        var html = await resp.Content.ReadAsStringAsync(ct);

        var title = Meta(html, "og:title");
        if (string.IsNullOrWhiteSpace(title))
            throw new ResolveFailedException("Spotify のトラック情報を読み取れませんでした。");
        // og:description is "Artist · Album · Song · Year" — the first segment is the artist.
        var artist = (Meta(html, "og:description") ?? "").Split('·', StringSplitOptions.TrimEntries)[0];
        return new[] { new MatchQuery(title, artist, 0, null) };
    }

    private static string? Meta(string html, string prop)
    {
        var e = Regex.Escape(prop);
        var m = Regex.Match(html, "<meta[^>]+(?:property|name)=\"" + e + "\"[^>]+content=\"([^\"]*)\"");
        if (!m.Success)
            m = Regex.Match(html, "<meta[^>]+content=\"([^\"]*)\"[^>]+(?:property|name)=\"" + e + "\"");
        return m.Success ? WebUtility.HtmlDecode(m.Groups[1].Value) : null;
    }
}
