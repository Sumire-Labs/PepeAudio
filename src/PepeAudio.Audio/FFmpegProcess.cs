// SPDX-License-Identifier: Apache-2.0
using System.Diagnostics;
using System.Globalization;
using Microsoft.Extensions.Logging;

namespace PepeAudio.Audio;

// Spawns FFmpeg to decode a source, apply an effect filtergraph, and emit raw
// 48kHz/16-bit/stereo PCM on stdout. The graph must end in a [out] pad.
public sealed class FFmpegProcess : IDisposable
{
    private readonly Process _proc;

    private FFmpegProcess(Process proc) => _proc = proc;

    public Stream Output => _proc.StandardOutput.BaseStream;

    public static FFmpegProcess Start(string ffmpegPath, string input, bool http, long seekMs,
        string filterComplex, ILogger log)
    {
        var psi = new ProcessStartInfo(ffmpegPath)
        {
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
        };
        void Add(params string[] a) { foreach (var x in a) psi.ArgumentList.Add(x); }

        Add("-hide_banner", "-loglevel", "warning", "-nostdin");
        if (http)
            Add("-reconnect", "1", "-reconnect_streamed", "1",
                "-reconnect_on_network_error", "1", "-reconnect_delay_max", "5");
        if (seekMs > 0)
            Add("-ss", (seekMs / 1000.0).ToString("0.###", CultureInfo.InvariantCulture));
        Add("-i", input);
        Add("-filter_complex", filterComplex, "-map", "[out]");
        Add("-f", "s16le", "-ar", "48000", "-ac", "2", "pipe:1");

        var proc = Process.Start(psi) ?? throw new InvalidOperationException("ffmpeg failed to start");
        _ = DrainStderrAsync(proc, log);
        return new FFmpegProcess(proc);
    }

    // At -loglevel warning ffmpeg's stderr is quiet unless something is wrong (e.g. a bad
    // filtergraph makes it print the error and exit with no PCM), so surface it — otherwise a
    // failing effect chain looks like a silent, empty track with no clue why.
    private static async Task DrainStderrAsync(Process p, ILogger log)
    {
        string err;
        try { err = await p.StandardError.ReadToEndAsync(); } catch { return; }
        if (!string.IsNullOrWhiteSpace(err))
            log.LogWarning("FFmpeg: {Error}", err.Length > 1000 ? err[^1000..] : err.Trim());
    }

    public void Dispose()
    {
        try { if (!_proc.HasExited) _proc.Kill(entireProcessTree: true); } catch { /* best effort */ }
        _proc.Dispose();
    }
}
