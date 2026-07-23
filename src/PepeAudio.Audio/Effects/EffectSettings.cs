// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core.Enums;
using PepeAudio.Core.Models;

namespace PepeAudio.Audio.Effects;

// Per-guild audio settings applied to the FFmpeg effect chain and the mixer.
public sealed class EffectSettings
{
    public bool AuraEnabled { get; set; } = true;
    public string PresetName { get; set; } = "Aura";
    public int Volume { get; set; } = 50;
    public NormalizationMode Normalization { get; set; } = NormalizationMode.Off;
    public int CrossfadeMs { get; set; }

    public static EffectSettings From(GuildSettings g) => new()
    {
        AuraEnabled = g.AuraEnabled,
        PresetName = g.PresetName,
        Volume = g.Volume,
        Normalization = g.Normalization,
        CrossfadeMs = g.CrossfadeMs,
    };
}
