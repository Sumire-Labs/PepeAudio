// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core.Enums;

namespace PepeAudio.Audio.Effects;

// Composes the FFmpeg -filter_complex graph (ending in [out]) from the settings.
public sealed class EffectChainBuilder
{
    // Convert to 48k with the high-quality soxr resampler (a no-op for already-48k sources, so
    // effectively free), then dither the float->s16 handoff to Discord to cut quantization noise.
    private const string OutFormat =
        "aformat=channel_layouts=stereo,aresample=osf=s16:dither_method=triangular_hp";

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
                return AuraConvolution.Build(preset, pre, OutFormat);
        }
        return $"{pre},{OutFormat}[out]";
    }

    private static string Normalization(NormalizationMode mode) => mode switch
    {
        NormalizationMode.LoudNorm => ",loudnorm=I=-14:TP=-1.5:LRA=11",
        NormalizationMode.DynAudNorm => ",dynaudnorm=f=250:g=15",
        _ => "",
    };
}
