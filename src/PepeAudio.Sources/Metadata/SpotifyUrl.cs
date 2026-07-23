// SPDX-License-Identifier: Apache-2.0
using System.Text.RegularExpressions;

namespace PepeAudio.Sources.Metadata;

public enum SpotifyKind { Track, Album, Playlist }

public sealed record SpotifyRef(SpotifyKind Kind, string Id);

public static partial class SpotifyUrl
{
    public static bool TryParse(string? input, out SpotifyRef reference)
    {
        reference = new SpotifyRef(SpotifyKind.Track, "");
        if (string.IsNullOrWhiteSpace(input)) return false;
        var m = Pattern().Match(input);
        if (!m.Success) return false;
        var kind = m.Groups[1].Value.ToLowerInvariant() switch
        {
            "album" => SpotifyKind.Album,
            "playlist" => SpotifyKind.Playlist,
            _ => SpotifyKind.Track,
        };
        reference = new SpotifyRef(kind, m.Groups[2].Value);
        return true;
    }

    public static bool IsSpotify(string? input)
        => input is not null &&
           (input.Contains("open.spotify.com", StringComparison.OrdinalIgnoreCase) ||
            input.StartsWith("spotify:", StringComparison.OrdinalIgnoreCase));

    [GeneratedRegex(@"(track|album|playlist)[/:]([A-Za-z0-9]+)")]
    private static partial Regex Pattern();
}
