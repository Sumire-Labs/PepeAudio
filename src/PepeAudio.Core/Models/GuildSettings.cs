// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core.Enums;

namespace PepeAudio.Core.Models;

public sealed class GuildSettings
{
    public ulong GuildId { get; init; }
    public bool AuraEnabled { get; set; } = true;
    public string PresetName { get; set; } = "Aura";
    public int Volume { get; set; } = 50;
    public NormalizationMode Normalization { get; set; } = NormalizationMode.Off;
    public int CrossfadeMs { get; set; }
    public ulong? DjRoleId { get; set; }
    public bool Autoplay { get; set; }
    public ulong? BoundTextChannelId { get; set; }
    public string Locale { get; set; } = "en-US";

    public static GuildSettings Default(ulong guildId) => new() { GuildId = guildId };
}
