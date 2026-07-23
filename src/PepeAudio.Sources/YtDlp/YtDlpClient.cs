// SPDX-License-Identifier: Apache-2.0
using System.Diagnostics;
using System.Globalization;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;

namespace PepeAudio.Sources.YtDlp;

public sealed record YtDlpTrack(string Id, string Title, string Artist, long DurationMs, string WebpageUrl, string? Thumbnail);

public sealed record YtDlpResolved(YtDlpTrack Track, string? StreamUrl);

public sealed record YtDlpCandidate(string Id, string Title, string? Channel, long DurationMs, string WebpageUrl);

public interface IYtDlpClient
{
    Task<YtDlpResolved?> GetTrackAsync(string input, CancellationToken ct);
    Task<string?> GetStreamUrlAsync(string input, CancellationToken ct);
    Task<IReadOnlyList<YtDlpCandidate>> SearchAsync(string query, int count, CancellationToken ct);
    Task<IReadOnlyList<YtDlpCandidate>> PlaylistEntriesAsync(string url, int max, CancellationToken ct);
}

public sealed class YtDlpClient : IYtDlpClient
{
    private readonly YtDlpOptions _opt;
    private readonly ILogger<YtDlpClient> _log;

    public YtDlpClient(IOptions<YtDlpOptions> opt, ILogger<YtDlpClient> log)
    {
        _opt = opt.Value;
        _log = log;
    }

    public async Task<string?> GetStreamUrlAsync(string input, CancellationToken ct)
    {
        var args = BaseArgs();
        args.AddRange(new[] { "--no-playlist", "-f", _opt.Format, "-g", "--", input });
        return FirstLine(await RunAsync(args, ct));
    }

    // Metadata and the direct stream URL in a single yt-dlp call (one process cold
    // start instead of two — roughly halves the pre-playback wait).
    public async Task<YtDlpResolved?> GetTrackAsync(string input, CancellationToken ct)
    {
        var args = BaseArgs();
        args.AddRange(new[] { "--no-playlist", "-f", _opt.Format,
            "--print", "%(id)s\t%(title)s\t%(uploader)s\t%(duration)s\t%(thumbnail)s\t%(webpage_url)s\t%(url)s", "--", input });
        var f = FirstLine(await RunAsync(args, ct))?.Split('\t');
        if (f is null || f.Length < 7) return null;
        var track = new YtDlpTrack(f[0], f[1], Clean(f[2]), Ms(f[3]), f[5], Clean(f[4]));
        return new YtDlpResolved(track, f[6] is "" or "NA" or "None" ? null : f[6]);
    }

    public Task<IReadOnlyList<YtDlpCandidate>> SearchAsync(string query, int count, CancellationToken ct)
        => EntriesAsync($"ytsearch{count}:{query}", count, ct);

    public Task<IReadOnlyList<YtDlpCandidate>> PlaylistEntriesAsync(string url, int max, CancellationToken ct)
        => EntriesAsync(url, max, ct);

    private async Task<IReadOnlyList<YtDlpCandidate>> EntriesAsync(string target, int max, CancellationToken ct)
    {
        var args = BaseArgs();
        args.AddRange(new[] { "--flat-playlist", "--playlist-end", max.ToString(CultureInfo.InvariantCulture),
            "--print", "%(id)s\t%(title)s\t%(channel)s\t%(duration)s\t%(url)s", "--", target });
        var list = new List<YtDlpCandidate>();
        foreach (var raw in (await RunAsync(args, ct)).Split('\n', StringSplitOptions.RemoveEmptyEntries))
        {
            var f = raw.Split('\t');
            if (f.Length < 5 || string.IsNullOrWhiteSpace(f[0])) continue;
            var web = f[4].StartsWith("http", StringComparison.OrdinalIgnoreCase)
                ? f[4] : $"https://www.youtube.com/watch?v={f[0]}";
            list.Add(new YtDlpCandidate(f[0], f[1], Clean(f[2]), Ms(f[3]), web));
        }
        return list;
    }

    private List<string> BaseArgs()
    {
        var args = new List<string> { "--no-warnings" };
        if (!string.IsNullOrWhiteSpace(_opt.PlayerClient))
        {
            args.Add("--extractor-args");
            args.Add($"youtube:player_client={_opt.PlayerClient}");
        }
        if (!string.IsNullOrWhiteSpace(_opt.PotBaseUrl))
        {
            args.Add("--extractor-args");
            args.Add($"youtubepot-bgutilhttp:base_url={_opt.PotBaseUrl}");
        }
        if (!string.IsNullOrWhiteSpace(_opt.CookiesFile))
        {
            args.Add("--cookies");
            args.Add(_opt.CookiesFile);
        }
        return args;
    }

    private async Task<string> RunAsync(List<string> args, CancellationToken ct)
    {
        var psi = new ProcessStartInfo(_opt.YtDlpPath)
        {
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
        };
        foreach (var a in args) psi.ArgumentList.Add(a);

        using var proc = Process.Start(psi) ?? throw new InvalidOperationException("yt-dlp failed to start");
        using var timeout = CancellationTokenSource.CreateLinkedTokenSource(ct);
        timeout.CancelAfter(TimeSpan.FromSeconds(_opt.ProcessTimeoutSeconds));

        var stdout = proc.StandardOutput.ReadToEndAsync(timeout.Token);
        var stderr = proc.StandardError.ReadToEndAsync(timeout.Token);
        try { await proc.WaitForExitAsync(timeout.Token); }
        catch (OperationCanceledException)
        {
            // Kill on BOTH timeout and caller cancellation, else the process is orphaned.
            try { if (!proc.HasExited) proc.Kill(entireProcessTree: true); } catch { }
            if (!ct.IsCancellationRequested) throw new TimeoutException("yt-dlp がタイムアウトしました");
            throw;
        }

        if (proc.ExitCode != 0)
        {
            _log.LogWarning("yt-dlp exit {Code}: {Err}", proc.ExitCode, (await stderr).Trim());
            return string.Empty;
        }
        return await stdout;
    }

    private static string? FirstLine(string s)
        => s.Split('\n', StringSplitOptions.RemoveEmptyEntries).FirstOrDefault()?.Trim() is { Length: > 0 } l ? l : null;

    private static string Clean(string? s) => s is null or "NA" or "None" ? "" : s;

    private static long Ms(string s)
        => double.TryParse(s, NumberStyles.Any, CultureInfo.InvariantCulture, out var sec) ? (long)(sec * 1000) : 0;
}
