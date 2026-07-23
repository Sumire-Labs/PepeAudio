// SPDX-License-Identifier: Apache-2.0
using Discord;
using Discord.WebSocket;
using PepeAudio.Discord;

namespace PepeAudio.Host.Sharding;

public static class ShardedClientFactory
{
    public static DiscordShardedClient Create(DiscordOptions opt)
    {
        var config = new DiscordSocketConfig
        {
            TotalShards = opt.TotalShards,
            GatewayIntents = GatewayIntents.Guilds | GatewayIntents.GuildVoiceStates,
            MessageCacheSize = 0,
            AlwaysDownloadUsers = false,
            LogLevel = LogSeverity.Info,
            // DAVE end-to-end voice encryption is mandatory since 2026-03-01; needs libdave.so.
            EnableVoiceDaveEncryption = true,
            // Measure the 3s interaction window from receipt, not the snowflake timestamp, so a
            // skewed host clock (common on sleeping dev machines) cannot spuriously fail DeferAsync.
            UseInteractionSnowflakeDate = false,
        };

        // Full set (this process owns every shard): let the client create them all.
        // A strict subset (multi-process fleet) uses the explicit-ids constructor.
        var ownsEveryShard = opt.ShardIds.Length == 0 || opt.ShardIds.Length >= opt.TotalShards;
        return ownsEveryShard
            ? new DiscordShardedClient(config)
            : new DiscordShardedClient(opt.ShardIds, config);
    }
}
