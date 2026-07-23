// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Core.Sharding;

// Describes this process's place in the shard fleet and routes guilds to shards.
public interface IShardTopology
{
    int TotalShards { get; }
    IReadOnlyCollection<int> OwnedShards { get; }
    int OwnerShardFor(ulong guildId);
    bool Owns(ulong guildId);
}

public sealed class ShardTopology : IShardTopology
{
    private readonly HashSet<int> _owned;

    public ShardTopology(int totalShards, IEnumerable<int> ownedShards)
    {
        TotalShards = Math.Max(1, totalShards);
        var owned = ownedShards.ToHashSet();
        // Empty or full set means this process owns every shard.
        _owned = owned.Count == 0 || owned.Count >= TotalShards
            ? Enumerable.Range(0, TotalShards).ToHashSet()
            : owned;
    }

    public int TotalShards { get; }
    public IReadOnlyCollection<int> OwnedShards => _owned;

    public int OwnerShardFor(ulong guildId) => GuildShardMap.ShardIdFor(guildId, TotalShards);

    public bool Owns(ulong guildId) => _owned.Contains(OwnerShardFor(guildId));
}
