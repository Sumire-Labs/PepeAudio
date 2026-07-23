// SPDX-License-Identifier: Apache-2.0
using System.Text.Json;
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Enums;
using Xunit;

namespace PepeAudio.Core.Tests;

public class CheckpointSerializationTests
{
    [Fact]
    public void Checkpoint_round_trips_through_json()
    {
        var track = new TrackInfo("Song", "Artist", SourceKind.YouTube, "https://y/1", 200_000, "thumb", false, 123UL);
        var track2 = new TrackInfo("Next", "Artist2", SourceKind.SoundCloud, "https://s/2", 180_000, null, false, 123UL);
        var current = new PlayableRef(SourceKind.YouTube, "https://y/1", true, track, NeedsResolution: true);
        var upcoming = new PlayableRef(SourceKind.SoundCloud, "https://s/2", true, track2, NeedsResolution: true);

        var cp = new PlayerCheckpoint(111UL, 222UL, 5_000, current, new[] { upcoming }, LoopMode.Queue, Shuffle: true);

        var back = JsonSerializer.Deserialize<PlayerCheckpoint>(JsonSerializer.Serialize(cp))!;

        Assert.Equal(cp.GuildId, back.GuildId);
        Assert.Equal(cp.VoiceChannelId, back.VoiceChannelId);
        Assert.Equal(cp.PositionMs, back.PositionMs);
        Assert.Equal(LoopMode.Queue, back.Loop);
        Assert.True(back.Shuffle);
        Assert.Equal(current, back.Current);       // PlayableRef/TrackInfo value equality
        Assert.Single(back.Queue);
        Assert.Equal(upcoming, back.Queue[0]);
        Assert.True(back.Current!.NeedsResolution); // stream re-resolved on resume, not stored
    }
}
