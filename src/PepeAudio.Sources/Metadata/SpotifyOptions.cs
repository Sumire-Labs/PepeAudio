// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Sources.Metadata;

public sealed class SpotifyOptions
{
    public const string Section = "Spotify";

    public string? ClientId { get; set; }
    public string? ClientSecret { get; set; }

    public bool Enabled => !string.IsNullOrWhiteSpace(ClientId) && !string.IsNullOrWhiteSpace(ClientSecret);
}
