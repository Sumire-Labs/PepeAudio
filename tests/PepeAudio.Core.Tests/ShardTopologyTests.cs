// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core.Sharding;
using Xunit;

namespace PepeAudio.Core.Tests;

public class ShardTopologyTests
{
    private const ulong GuildOnShard2 = 2UL << 22; // (g >> 22) % 4 == 2

    [Fact]
    public void Owns_only_its_subset()
    {
        var lower = new ShardTopology(4, new[] { 0, 1 });
        var upper = new ShardTopology(4, new[] { 2, 3 });

        Assert.Equal(2, lower.OwnerShardFor(GuildOnShard2));
        Assert.False(lower.Owns(GuildOnShard2));
        Assert.True(upper.Owns(GuildOnShard2));
    }

    [Fact]
    public void Empty_or_full_set_owns_everything()
    {
        var single = new ShardTopology(1, Array.Empty<int>());
        Assert.True(single.Owns(GuildOnShard2));
        Assert.True(single.Owns(0));

        var full = new ShardTopology(4, new[] { 0, 1, 2, 3 });
        Assert.Equal(4, full.OwnedShards.Count);
        Assert.True(full.Owns(GuildOnShard2));
    }
}
