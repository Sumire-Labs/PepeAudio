// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Cache;

// Owner-token compare-and-swap Lua: act only while the stored value still equals
// the caller's token. Shared by the voice lock and the shard registry.
internal static class ValkeyLua
{
    public const string CompareAndPExpire =
        "if redis.call('get', KEYS[1]) == ARGV[1] then return redis.call('pexpire', KEYS[1], ARGV[2]) else return 0 end";
    public const string CompareAndDelete =
        "if redis.call('get', KEYS[1]) == ARGV[1] then return redis.call('del', KEYS[1]) else return 0 end";
}
