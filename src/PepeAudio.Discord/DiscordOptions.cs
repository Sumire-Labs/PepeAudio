// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Discord;

public sealed class DiscordOptions
{
    public const string Section = "Discord";

    public string Token { get; set; } = "";
    // Nullable so a blank env var (DISCORD__DEVGUILDID=) binds to null instead of failing.
    public ulong? DevGuildId { get; set; }
    public bool UseGlobalCommands { get; set; }
    public int TotalShards { get; set; } = 1;
    public int[] ShardIds { get; set; } = { 0 };
}
