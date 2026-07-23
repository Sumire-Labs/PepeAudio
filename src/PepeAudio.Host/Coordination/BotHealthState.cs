// SPDX-License-Identifier: Apache-2.0
using System.Collections.Concurrent;

namespace PepeAudio.Host.Coordination;

// Tracks gateway readiness so health probes can report liveness/readiness.
public sealed class BotHealthState
{
    private readonly ConcurrentDictionary<int, byte> _ready = new();

    public bool LoggedIn { get; set; }
    public int ExpectedShards { get; set; }
    public int ReadyShards => _ready.Count;

    public bool Ready => LoggedIn && ExpectedShards > 0 && ReadyShards >= ExpectedShards;

    public void MarkShardReady(int shardId) => _ready[shardId] = 1;

    public void Reset()
    {
        LoggedIn = false;
        _ready.Clear();
    }
}
