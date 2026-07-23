// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Core;

// Deterministic guild -> shard mapping, identical to Discord's routing and
// Discord.Net's GetShardIdFor. Never use plain guildId % totalShards.
public static class GuildShardMap
{
    public static int ShardIdFor(ulong guildId, int totalShards)
        => (int)((guildId >> 22) % (uint)totalShards);
}
