// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Sources.Metadata;

public sealed class AppleMusicOptions
{
    public const string Section = "AppleMusic";

    public string? TeamId { get; set; }
    public string? KeyId { get; set; }
    public string? PrivateKeyPath { get; set; }
    public string Storefront { get; set; } = "us";

    public bool Enabled => !string.IsNullOrWhiteSpace(TeamId) && !string.IsNullOrWhiteSpace(KeyId)
        && !string.IsNullOrWhiteSpace(PrivateKeyPath) && File.Exists(PrivateKeyPath);
}
