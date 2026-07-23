// SPDX-License-Identifier: Apache-2.0
using System.Diagnostics;
using System.Globalization;
using System.Security.Cryptography;
using System.Text.Json;
using System.Text.RegularExpressions;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;

namespace PepeAudio.Audio.Effects;

public interface IHeSuViPresetLibrary
{
    HeSuViPreset? Get(string name);
    // Fallback so Aura still applies when the stored preset name matches no file on disk.
    HeSuViPreset? Default { get; }
    IReadOnlyCollection<HeSuViPreset> All { get; }
}

// Scans the presets directory for HeSuVi WAV impulse responses, reads their channel count from the
// WAV header, and measures each one's loudness makeup once by running the real convolution graph
// on decorrelated pink noise and comparing input vs output RMS via ffmpeg astats. Measured values
// are cached (keyed by file hash) so later startups skip the ffmpeg runs.
public sealed class HeSuViPresetLibrary : IHeSuViPresetLibrary
{
    private const double Headroom = 4;        // keep a hot master off the limiter after the boost
    private const int MeasureTimeoutMs = 20_000;
    private const string CacheFileName = ".makeup-cache.json";
    private const int CacheVersion = 1;       // bump when the measure graph semantics change
    // Two decorrelated pink-noise channels — a correlated signal under-reads the convolution loss.
    private const string PinkNoise =
        "anoisesrc=color=pink:amplitude=0.2:duration=2:seed=1:sample_rate=48000[l];" +
        "anoisesrc=color=pink:amplitude=0.2:duration=2:seed=2:sample_rate=48000[r];" +
        "[l][r]join=inputs=2:channel_layout=stereo";

    private readonly Dictionary<string, HeSuViPreset> _presets = new(StringComparer.OrdinalIgnoreCase);

    public HeSuViPresetLibrary(IOptions<AudioOptions> opt, ILogger<HeSuViPresetLibrary> log)
    {
        var dir = opt.Value.PresetsDir;
        if (!Directory.Exists(dir))
        {
            log.LogInformation("Presets directory '{Dir}' not found; Aura runs pass-through until a preset is added.", dir);
            return;
        }

        var ffmpeg = opt.Value.FFmpegPath;
        var cachePath = Path.Combine(dir, CacheFileName);
        var cache = LoadCache(cachePath);
        var dirty = false;
        // The pink-noise reference RMS is only needed on a cache miss; measured lazily so a fully
        // cached (or empty) directory spawns no ffmpeg at all.
        var inputRms = new Lazy<double?>(() => MeasureRms(ffmpeg,
            new[] { "-hide_banner", "-f", "lavfi", "-i", PinkNoise, "-af", "astats=metadata=0", "-f", "null", "-" }, log));

        foreach (var path in Directory.EnumerateFiles(dir, "*.wav"))
        {
            var (channels, sampleRate) = WavHeader.ReadFormat(path);
            var name = Path.GetFileNameWithoutExtension(path);
            var preset = new HeSuViPreset(name, Path.GetFullPath(path), channels);
            if (preset.IsSupported)
            {
                if (sampleRate != 48000)
                    log.LogWarning("Preset '{Name}' is {Rate} Hz (expected 48000); its IR is soxr-resampled at play time — prefer a native 48 kHz file.", name, sampleRate);
                var sha = Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(path)));
                if (cache.Entries.TryGetValue(name, out var hit) && hit.Sha256 == sha)
                    preset = preset with { MakeupDb = hit.MakeupDb };
                else if (MeasureMakeup(ffmpeg, preset, inputRms.Value, log) is { } measured)
                {
                    preset = preset with { MakeupDb = measured };
                    cache.Entries[name] = new MakeupEntry { Sha256 = sha, MakeupDb = measured };
                    dirty = true;
                }
                else
                    log.LogWarning("Makeup measurement failed for preset '{Name}'; using default {Db} dB (not cached).", name, preset.MakeupDb);
                log.LogInformation("Preset '{Name}' ({Ch}ch): makeup {Db} dB.", name, channels, preset.MakeupDb);
            }
            else
                log.LogWarning("Preset '{Name}' has {Ch} channels (unsupported); it will pass through.", name, channels);
            _presets[name] = preset;
        }

        foreach (var stale in cache.Entries.Keys.Where(k => !_presets.ContainsKey(k)).ToList())
        {
            cache.Entries.Remove(stale);
            dirty = true;
        }
        if (dirty) SaveCache(cachePath, cache, log);

        // Prefer the configured default name, then name order — deterministic across filesystems
        // (Linux readdir order is arbitrary, so "first file found" would vary per deployment).
        Default = _presets.Values
            .Where(p => p.IsSupported)
            .OrderByDescending(p => string.Equals(p.Name, opt.Value.DefaultPreset, StringComparison.OrdinalIgnoreCase))
            .ThenBy(p => p.Name, StringComparer.OrdinalIgnoreCase)
            .FirstOrDefault();
        log.LogInformation("Loaded {Count} HeSuVi preset(s); default '{Default}'.", _presets.Count, Default?.Name ?? "(none)");
    }

    public HeSuViPreset? Get(string name)
        => _presets.TryGetValue(name, out var p) ? p : null;

    public HeSuViPreset? Default { get; }

    public IReadOnlyCollection<HeSuViPreset> All => _presets.Values;

    private static double? MeasureMakeup(string ffmpeg, HeSuViPreset preset, double? inputRms, ILogger log)
    {
        if (inputRms is not { } inRms) return null;
        var outRms = MeasureRms(ffmpeg, new[]
        {
            "-hide_banner", "-f", "lavfi", "-i", PinkNoise,
            "-filter_complex", AuraConvolution.BuildMeasureGraph(preset), "-map", "[out]", "-f", "null", "-",
        }, log);
        return outRms is { } o ? Math.Clamp(Math.Round(inRms - o - Headroom, 1), 0, 30) : null;
    }

    // Runs ffmpeg, reads the last astats "RMS level dB" (the Overall figure) from stderr.
    private static double? MeasureRms(string ffmpeg, string[] args, ILogger log)
    {
        try
        {
            var psi = new ProcessStartInfo(ffmpeg) { RedirectStandardError = true, UseShellExecute = false };
            foreach (var a in args) psi.ArgumentList.Add(a);
            using var proc = Process.Start(psi);
            if (proc is null) return null;
            // Drain stderr concurrently: reading after exit can deadlock on a full pipe, and a
            // blocking read before WaitForExit would make the timeout unreachable if ffmpeg hangs.
            var errTask = proc.StandardError.ReadToEndAsync();
            if (!proc.WaitForExit(MeasureTimeoutMs)) { try { proc.Kill(entireProcessTree: true); } catch { } return null; }
            var err = errTask.GetAwaiter().GetResult();
            var m = Regex.Matches(err, @"RMS level dB:\s*(-?[\d.]+)");
            return m.Count > 0 && double.TryParse(m[^1].Groups[1].Value, NumberStyles.Float, CultureInfo.InvariantCulture, out var v)
                ? v : null;
        }
        catch (Exception ex)
        {
            log.LogDebug(ex, "Makeup measurement failed; using default.");
            return null;
        }
    }

    private static MakeupCache LoadCache(string path)
    {
        try
        {
            if (File.Exists(path) &&
                JsonSerializer.Deserialize<MakeupCache>(File.ReadAllText(path)) is { } cache &&
                cache.Version == CacheVersion)
                return cache;
        }
        catch { /* corrupt cache -> re-measure */ }
        return new MakeupCache { Version = CacheVersion };
    }

    private static void SaveCache(string path, MakeupCache cache, ILogger log)
    {
        try
        {
            File.WriteAllText(path, JsonSerializer.Serialize(cache, new JsonSerializerOptions { WriteIndented = true }));
        }
        catch (Exception ex)
        {
            log.LogDebug(ex, "Could not write makeup cache (read-only presets dir?); measurements rerun next start.");
        }
    }

    private sealed class MakeupCache
    {
        public int Version { get; set; }
        public Dictionary<string, MakeupEntry> Entries { get; set; } = new(StringComparer.OrdinalIgnoreCase);
    }

    private sealed class MakeupEntry
    {
        public string Sha256 { get; set; } = "";
        public double MakeupDb { get; set; }
    }
}
