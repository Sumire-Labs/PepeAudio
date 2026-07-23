// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Audio.Effects;
using PepeAudio.Core.Enums;
using Xunit;

namespace PepeAudio.Audio.Tests;

public class EffectChainTests
{
    private sealed class StubLibrary : IHeSuViPresetLibrary
    {
        private readonly HeSuViPreset? _get;
        private readonly HeSuViPreset? _default;
        public StubLibrary(HeSuViPreset? preset) { _get = preset; _default = preset; }
        public StubLibrary(HeSuViPreset? getResult, HeSuViPreset? defaultResult) { _get = getResult; _default = defaultResult; }
        public HeSuViPreset? Get(string name) => _get;
        public HeSuViPreset? Default => _default;
        public IReadOnlyCollection<HeSuViPreset> All => _get is null ? Array.Empty<HeSuViPreset>() : new[] { _get };
    }

    [Fact]
    public void Passthrough_when_no_preset_and_always_ends_in_out()
    {
        var chain = new EffectChainBuilder(new StubLibrary(null));
        var graph = chain.Build(new EffectSettings { AuraEnabled = true });
        Assert.EndsWith("[out]", graph);
        Assert.Contains("aformat", graph);
        Assert.DoesNotContain("afir", graph);
    }

    [Fact]
    public void Falls_back_to_available_preset_when_stored_name_missing()
    {
        // Get() misses (stored "Aura" != file name) but a supported preset exists -> still convolves.
        var tmp = Path.Combine(Path.GetTempPath(), $"pepe_{Guid.NewGuid():N}.wav");
        File.WriteAllBytes(tmp, new byte[64]);
        try
        {
            var fallback = new HeSuViPreset("Aura_Halo_1.4", tmp, 4);
            var chain = new EffectChainBuilder(new StubLibrary(getResult: null, defaultResult: fallback));
            var graph = chain.Build(new EffectSettings { AuraEnabled = true, PresetName = "Aura" });
            Assert.Contains("afir=", graph); // convolution applied via fallback, not passthrough
        }
        finally { File.Delete(tmp); }
    }

    [Fact]
    public void Includes_normalization_filter()
    {
        var chain = new EffectChainBuilder(new StubLibrary(null));
        var graph = chain.Build(new EffectSettings { AuraEnabled = false, Normalization = NormalizationMode.LoudNorm });
        // loudnorm forces 192k internally, so it must be followed immediately by the soxr return to 48k.
        Assert.Contains("loudnorm=I=-14:TP=-1.5:LRA=11,aresample=48000:resampler=soxr", graph);
    }

    [Fact]
    public void Output_stage_pins_48k_with_soxr_and_dither()
    {
        var chain = new EffectChainBuilder(new StubLibrary(null));
        var graph = chain.Build(new EffectSettings { AuraEnabled = false });
        Assert.Contains("aresample=48000:resampler=soxr:precision=28:osf=s16:dither_method=triangular_hp", graph);
    }

    [Fact]
    public void True_stereo_graph_uses_afir_and_pan()
    {
        var preset = new HeSuViPreset("Aura", "/presets/aura.wav", 4);
        var graph = AuraConvolution.Build(preset, "aresample=48000", "aformat=x");
        Assert.Contains("pan=4C", graph);
        Assert.Contains("afir=", graph);
        // IR branch pinned to 48k with soxr so a non-48k IR never falls to the default swr.
        Assert.Contains("amovie=/presets/aura.wav,aresample=48000:resampler=soxr", graph);
        // Canonical true-stereo sum (unit gains), not pan's renormalized '<'.
        Assert.Contains("pan=stereo|FL=c0+c2|FR=c1+c3", graph);
        Assert.EndsWith("[out]", graph);
    }

    [Fact]
    public void Makeup_tail_disables_alimiter_auto_level()
    {
        var preset = new HeSuViPreset("Stereo", "/presets/s.wav", 2, MakeupDb: 12.5);
        var graph = AuraConvolution.Build(preset, "aresample=48000", "aformat=x");
        Assert.Contains("volume=12.5dB", graph);
        // Default alimiter auto-level rescales by 1/limit and would cancel the 0.95 headroom.
        Assert.Contains("alimiter=limit=0.95:level=false:latency=1", graph);
    }

    [Fact]
    public void Plain_stereo_graph_skips_the_pan_split()
    {
        var preset = new HeSuViPreset("Stereo", "/presets/s.wav", 2);
        var graph = AuraConvolution.Build(preset, "aresample=48000", "aformat=x");
        Assert.DoesNotContain("pan=4C", graph);
        Assert.Contains("afir=", graph);
    }

    [Fact]
    public void Immersive_14ch_graph_uses_headphone_hrir_stereo()
    {
        var preset = new HeSuViPreset("Aura_Halo_1.4", "/presets/a.wav", 14);
        var graph = AuraConvolution.Build(preset, "aresample=48000", "aformat=x");
        Assert.Contains("headphone=map=FL|FR|SL|SR:hrir=stereo", graph);
        Assert.Contains("join=inputs=4", graph);
        Assert.Contains("c0=c8|c1=c7", graph); // FR uses the real HeSuVi channels (right-ear first)
        Assert.EndsWith("[out]", graph);
    }
}
