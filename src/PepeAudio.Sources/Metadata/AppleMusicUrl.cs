// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Sources.Metadata;

public enum AppleKind { Song, Album, Playlist }

public sealed record AppleRef(AppleKind Kind, string Id, string Storefront);

public static class AppleMusicUrl
{
    public static bool IsAppleMusic(string? input)
        => input is not null && input.Contains("music.apple.com", StringComparison.OrdinalIgnoreCase);

    public static bool TryParse(string? input, string defaultStorefront, out AppleRef reference)
    {
        reference = new AppleRef(AppleKind.Song, "", defaultStorefront);
        if (!Uri.TryCreate(input, UriKind.Absolute, out var uri)) return false;

        var segments = uri.AbsolutePath.Trim('/').Split('/', StringSplitOptions.RemoveEmptyEntries);
        if (segments.Length == 0) return false;

        var storefront = segments[0].Length == 2 ? segments[0] : defaultStorefront;
        var songId = QueryValue(uri.Query, "i");
        var last = segments[^1];

        if (!string.IsNullOrEmpty(songId))
            reference = new AppleRef(AppleKind.Song, songId, storefront);
        else if (uri.AbsolutePath.Contains("/album/", StringComparison.OrdinalIgnoreCase))
            reference = new AppleRef(AppleKind.Album, last, storefront);
        else if (uri.AbsolutePath.Contains("/playlist/", StringComparison.OrdinalIgnoreCase))
            reference = new AppleRef(AppleKind.Playlist, last, storefront);
        else if (uri.AbsolutePath.Contains("/song/", StringComparison.OrdinalIgnoreCase))
            reference = new AppleRef(AppleKind.Song, last, storefront);
        else
            return false;

        return !string.IsNullOrEmpty(reference.Id);
    }

    private static string? QueryValue(string query, string key)
    {
        foreach (var pair in query.TrimStart('?').Split('&', StringSplitOptions.RemoveEmptyEntries))
        {
            var eq = pair.IndexOf('=');
            if (eq > 0 && pair[..eq] == key) return Uri.UnescapeDataString(pair[(eq + 1)..]);
        }
        return null;
    }
}
