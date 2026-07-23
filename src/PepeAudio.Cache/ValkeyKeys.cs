// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Cache;

// Canonical Valkey keyspace registry (single 'pepe:' prefix).
public static class ValkeyKeys
{
    public const string Prefix = "pepe:";

    public static string ShardOwner(int shardId) => $"{Prefix}shard:owner:{shardId}";
    public static string VoiceLock(ulong guildId) => $"{Prefix}lock:voice:{guildId}";
    public static string Player(ulong guildId) => $"{Prefix}player:{guildId}";
    public static string ControlStream(int shardId) => $"{Prefix}control:shard:{shardId}";
    public static string Track(string hash) => $"{Prefix}track:{hash}";
    public const string LockMigrate = Prefix + "lock:migrate";
    public const string LockCommandReg = Prefix + "lock:cmdreg";
}
