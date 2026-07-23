// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core.Enums;

namespace PepeAudio.Audio.Effects;

// Composes the FFmpeg -filter_complex graph (ending in [out]) from the settings.
public sealed class EffectChainBuilder
{
    // Pin [out] to 48k via soxr (a no-op for already-48k graphs, so effectively free) and dither
    // the float->s16 handoff to Discord. Naming the rate here keeps ffmpeg's default-quality
    // auto-resampler out of the path when an upstream filter changed the rate.
    private const string OutFormat =
        "aformat=channel_layouts=stereo,aresample=48000:resampler=soxr:precision=28:osf=s16:dither_method=triangular_hp";

    private readonly IHeSuViPresetLibrary _presets;

    public EffectChainBuilder(IHeSuViPresetLibrary presets) => _presets = presets;

    public string Build(EffectSettings fx)
    {
        var pre = AuraConvolution.SoxrResample + Normalization(fx.Normalization);

        if (fx.AuraEnabled)
        {
            // Fall back to any available preset so a stored name that matches no file
            // (e.g. default "Aura" vs a differently-named .wav) still convolves instead of passing through.
            var preset = _presets.Get(fx.PresetName) ?? _presets.Default;
            if (preset is { IsSupported: true } && File.Exists(preset.Path))
                return AuraConvolution.Build(preset, pre, OutFormat, fx.Aura360Enabled);
        }
        // Standalone 360° (no BRIR): tone + Haas widen/depth, capped by the limiter because the
        // bypass path has no makeup tail and the bass boost would otherwise clip a hot master.
        return fx.Aura360Enabled
            ? $"{pre},{AuraConvolution.Aura360Tone},{AuraConvolution.Aura360Widen},{AuraConvolution.Limiter},{OutFormat}[out]"
            : $"{pre},{OutFormat}[out]";
    }

    // loudnorm runs at 192k internally (its in/out are forced there), so drop straight back to 48k
    // with soxr — otherwise the whole convolution runs at 192k and the final rate conversion falls
    // to the default swr the ffmpeg output stage auto-inserts.
    private static string Normalization(NormalizationMode mode) => mode switch
    {
        NormalizationMode.LoudNorm => ",loudnorm=I=-14:TP=-1.5:LRA=11," + AuraConvolution.SoxrResample,
        NormalizationMode.DynAudNorm => ",dynaudnorm=f=250:g=15",
        _ => "",
    };
}
