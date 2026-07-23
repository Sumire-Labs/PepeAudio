// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core;
using Xunit;

namespace PepeAudio.Core.Tests;

public class GuildShardMapTests
{
    [Theory]
    [InlineData(81384788765712384UL, 4)]
    [InlineData(41771983423143937UL, 16)]
    [InlineData(0UL, 1)]
    public void Matches_discord_routing_formula(ulong guildId, int totalShards)
    {
        var expected = (int)((guildId >> 22) % (uint)totalShards);
        Assert.Equal(expected, GuildShardMap.ShardIdFor(guildId, totalShards));
    }

    [Fact]
    public void Stays_in_range()
    {
        const int total = 8;
        for (ulong g = 0; g < 10_000; g += 137)
        {
            var shard = GuildShardMap.ShardIdFor(g << 22, total);
            Assert.InRange(shard, 0, total - 1);
        }
    }
}
