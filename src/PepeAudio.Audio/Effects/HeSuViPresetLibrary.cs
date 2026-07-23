// SPDX-License-Identifier: Apache-2.0
using System.Diagnostics;
using System.Globalization;
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
// WAV header, and measures each one's loudness makeup once at load by running the real convolution
// graph on decorrelated pink noise and comparing input vs output RMS via ffmpeg astats.
public sealed class HeSuViPresetLibrary : IHeSuViPresetLibrary
{
    private const double Headroom = 4;        // keep a hot master off the limiter after the boost
    private const double DefaultMakeup = 16;  // used when ffmpeg/astats is unavailable
    private const int MeasureTimeoutMs = 20_000;
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
        var inputRms = MeasureRms(ffmpeg,
            new[] { "-hide_banner", "-f", "lavfi", "-i", PinkNoise, "-af", "astats=metadata=0", "-f", "null", "-" }, log);

        foreach (var path in Directory.EnumerateFiles(dir, "*.wav"))
        {
            var channels = WavHeader.ReadChannels(path);
            var name = Path.GetFileNameWithoutExtension(path);
            var preset = new HeSuViPreset(name, Path.GetFullPath(path), channels);
            if (preset.IsSupported)
            {
                preset = preset with { MakeupDb = MeasureMakeup(ffmpeg, preset, inputRms, log) };
                log.LogInformation("Preset '{Name}' ({Ch}ch): makeup {Db} dB.", name, channels, preset.MakeupDb);
            }
            else
                log.LogWarning("Preset '{Name}' has {Ch} channels (unsupported); it will pass through.", name, channels);
            _presets[name] = preset;
        }
        Default = _presets.Values.FirstOrDefault(p => p.IsSupported);
        log.LogInformation("Loaded {Count} HeSuVi preset(s).", _presets.Count);
    }

    public HeSuViPreset? Get(string name)
        => _presets.TryGetValue(name, out var p) ? p : null;

    public HeSuViPreset? Default { get; }

    public IReadOnlyCollection<HeSuViPreset> All => _presets.Values;

    private static double MeasureMakeup(string ffmpeg, HeSuViPreset preset, double? inputRms, ILogger log)
    {
        if (inputRms is not { } inRms) return DefaultMakeup;
        var outRms = MeasureRms(ffmpeg, new[]
        {
            "-hide_banner", "-f", "lavfi", "-i", PinkNoise,
            "-filter_complex", AuraConvolution.BuildMeasureGraph(preset), "-map", "[out]", "-f", "null", "-",
        }, log);
        return outRms is { } o ? Math.Clamp(Math.Round(inRms - o - Headroom, 1), 0, 30) : DefaultMakeup;
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
            var err = proc.StandardError.ReadToEnd();
            if (!proc.WaitForExit(MeasureTimeoutMs)) { try { proc.Kill(entireProcessTree: true); } catch { } return null; }
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
}
