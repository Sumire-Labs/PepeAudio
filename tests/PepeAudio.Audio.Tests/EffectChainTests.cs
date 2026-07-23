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
    public void Aura360_renders_through_the_rear_brir_speakers()
    {
        var tmp = Path.Combine(Path.GetTempPath(), $"pepe_{Guid.NewGuid():N}.wav");
        File.WriteAllBytes(tmp, new byte[64]);
        try
        {
            var chain = new EffectChainBuilder(new StubLibrary(new HeSuViPreset("Aura", tmp, 14)));
            var graph = chain.Build(new EffectSettings { AuraEnabled = true, Aura360Enabled = true });
            // Tone stage sits before the convolution.
            var tone = graph.IndexOf("equalizer=f=60", StringComparison.Ordinal);
            var convolution = graph.IndexOf("headphone=", StringComparison.Ordinal);
            Assert.True(tone >= 0 && convolution > tone, "tone stage must sit in the pre-convolution branch");
            // Six speakers, rears from the real HeSuVi back pairs (BL c4/c5, BR right-ear-first c12/c11).
            Assert.Contains("join=inputs=6", graph);
            Assert.Contains("headphone=map=FL|FR|SL|SR|BL|BR:hrir=stereo", graph);
            Assert.Contains("c0=c4|c1=c5", graph);
            Assert.Contains("c0=c12|c1=c11", graph);
            // Asymmetric rear pre-delays + air-absorption lowpass = diffuse, distant rear field.
            Assert.Contains("adelay=15", graph);
            Assert.Contains("adelay=21", graph);
            Assert.Contains("lowpass=f=6500", graph);
            // The Haas widen stage is only for rear-less paths; real rears replace it here.
            Assert.DoesNotContain("stereowiden", graph);
        }
        finally { File.Delete(tmp); }
    }

    [Fact]
    public void Aura360_uses_its_own_measured_makeup()
    {
        var preset = new HeSuViPreset("Aura", "/presets/a.wav", 14, MakeupDb: 10, Makeup360Db: 20);
        Assert.Contains("volume=10dB", AuraConvolution.Build(preset, "aresample=48000", "aformat=x"));
        Assert.Contains("volume=20dB", AuraConvolution.Build(preset, "aresample=48000", "aformat=x", aura360: true));
    }

    [Fact]
    public void Aura360_on_a_rearless_preset_falls_back_to_haas_widening()
    {
        var preset = new HeSuViPreset("Stereo", "/presets/s.wav", 4);
        var graph = AuraConvolution.Build(preset, "aresample=48000", "aformat=x", aura360: true);
        Assert.Contains("stereowiden", graph);
        Assert.Contains("afir=", graph);
    }

    [Fact]
    public void Standalone_aura360_is_limited_before_s16()
    {
        var chain = new EffectChainBuilder(new StubLibrary(null));
        var graph = chain.Build(new EffectSettings { AuraEnabled = false, Aura360Enabled = true });
        Assert.Contains("equalizer=f=60", graph);
        Assert.Contains("stereowiden", graph);
        Assert.Contains("crossfeed=strength=0.4", graph);
        // No makeup tail downstream in the bypass path, so the bass boost needs its own limiter.
        Assert.Contains("alimiter=limit=0.95:level=false:latency=1", graph);
        Assert.DoesNotContain("afir", graph);
    }

    [Fact]
    public void Aura360_off_leaves_the_chain_untouched()
    {
        var chain = new EffectChainBuilder(new StubLibrary(null));
        var graph = chain.Build(new EffectSettings { AuraEnabled = false, Aura360Enabled = false });
        Assert.DoesNotContain("stereowiden", graph);
        Assert.DoesNotContain("equalizer", graph);
        Assert.DoesNotContain("alimiter", graph);
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
