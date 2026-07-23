// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Sources.YtDlp;

public sealed class YtDlpOptions
{
    public const string Section = "Sources";

    public string YtDlpPath { get; set; } = "yt-dlp";
    // yt-dlp's default multi-client set; a single client like "tv" can hit DRM-only
    // formats on some videos. Blank omits the override entirely.
    public string PlayerClient { get; set; } = "default";
    public string Format { get; set; } = "bestaudio[acodec=opus]/bestaudio/best";
    public string? PotBaseUrl { get; set; }
    public string? CookiesFile { get; set; }
    public int MaxPlaylistItems { get; set; } = 500;
    public int SearchCount { get; set; } = 8;
    public int ProcessTimeoutSeconds { get; set; } = 30;
}
