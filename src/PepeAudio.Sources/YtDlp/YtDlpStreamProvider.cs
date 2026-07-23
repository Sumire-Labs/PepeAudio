// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Exceptions;

namespace PepeAudio.Sources.YtDlp;

// Resolves a track's stream URL just before playback. Directly-playable inputs
// (direct URL / attachment) pass through; extractor sources are resolved via yt-dlp.
public sealed class YtDlpStreamProvider : IStreamProvider
{
    private readonly IYtDlpClient _ytdlp;

    public YtDlpStreamProvider(IYtDlpClient ytdlp) => _ytdlp = ytdlp;

    public async Task<string> ResolveStreamAsync(PlayableRef track, CancellationToken ct)
    {
        if (!track.NeedsResolution)
            return track.Input;
        if (track.Prefetched is { } pre && IsFresh(pre))
            return pre;
        return await _ytdlp.GetStreamUrlAsync(track.Input, ct)
            ?? throw new ResolveFailedException($"'{track.Info.Title}' のストリームを取得できませんでした。");
    }

    // googlevideo URLs embed an `expire` unix-seconds param. Reuse a prefetched URL
    // only while it keeps comfortable headroom; otherwise re-resolve just in time.
    private static bool IsFresh(string url)
    {
        const string marker = "expire=";
        var i = url.IndexOf(marker, StringComparison.Ordinal);
        if (i < 0) return false; // no verifiable expiry (e.g. SoundCloud) -> re-resolve just in time
        var span = url.AsSpan(i + marker.Length);
        var amp = span.IndexOf('&');
        if (amp >= 0) span = span[..amp];
        return long.TryParse(span, out var exp)
            && DateTimeOffset.FromUnixTimeSeconds(exp) - DateTimeOffset.UtcNow > TimeSpan.FromMinutes(5);
    }
}
