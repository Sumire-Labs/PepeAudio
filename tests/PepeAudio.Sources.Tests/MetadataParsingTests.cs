// SPDX-License-Identifier: Apache-2.0
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using PepeAudio.Sources.Metadata;
using Xunit;

namespace PepeAudio.Sources.Tests;

public class MetadataParsingTests
{
    [Theory]
    [InlineData("https://open.spotify.com/track/6rqhFgbbKwnb9MLmUQDhG6", SpotifyKind.Track, "6rqhFgbbKwnb9MLmUQDhG6")]
    [InlineData("https://open.spotify.com/intl-ja/album/1DFixLWuPkv3KT3TnV35m3", SpotifyKind.Album, "1DFixLWuPkv3KT3TnV35m3")]
    [InlineData("spotify:playlist:37i9dQZF1DXcBWIGoYBM5M", SpotifyKind.Playlist, "37i9dQZF1DXcBWIGoYBM5M")]
    public void Parses_spotify_urls(string url, SpotifyKind kind, string id)
    {
        Assert.True(SpotifyUrl.TryParse(url, out var r));
        Assert.Equal(kind, r.Kind);
        Assert.Equal(id, r.Id);
    }

    [Fact]
    public void Rejects_non_spotify()
        => Assert.False(SpotifyUrl.TryParse("https://youtube.com/watch?v=abc", out _));

    [Theory]
    [InlineData("https://music.apple.com/us/album/x/1546883292?i=1546883295", AppleKind.Song, "1546883295", "us")]
    [InlineData("https://music.apple.com/jp/album/x/1546883292", AppleKind.Album, "1546883292", "jp")]
    [InlineData("https://music.apple.com/us/playlist/x/pl.abc123", AppleKind.Playlist, "pl.abc123", "us")]
    public void Parses_apple_urls(string url, AppleKind kind, string id, string storefront)
    {
        Assert.True(AppleMusicUrl.TryParse(url, "us", out var r));
        Assert.Equal(kind, r.Kind);
        Assert.Equal(id, r.Id);
        Assert.Equal(storefront, r.Storefront);
    }

    [Fact]
    public void Apple_token_is_a_valid_es256_jwt()
    {
        using var ec = ECDsa.Create(ECCurve.NamedCurves.nistP256);
        var pem = ec.ExportPkcs8PrivateKeyPem();
        var now = DateTimeOffset.FromUnixTimeSeconds(1_700_000_000);

        var jwt = AppleDeveloperToken.Create("TEAMID", "KEYID", pem, now, TimeSpan.FromDays(150));
        var parts = jwt.Split('.');
        Assert.Equal(3, parts.Length);

        using var header = JsonDocument.Parse(Decode(parts[0]));
        Assert.Equal("ES256", header.RootElement.GetProperty("alg").GetString());
        Assert.Equal("KEYID", header.RootElement.GetProperty("kid").GetString());

        var signed = Encoding.ASCII.GetBytes($"{parts[0]}.{parts[1]}");
        Assert.True(ec.VerifyData(signed, Decode(parts[2]), HashAlgorithmName.SHA256,
            DSASignatureFormat.IeeeP1363FixedFieldConcatenation));
    }

    private static byte[] Decode(string segment)
    {
        var s = segment.Replace('-', '+').Replace('_', '/');
        s = s.PadRight(s.Length + (4 - s.Length % 4) % 4, '=');
        return Convert.FromBase64String(s);
    }
}
