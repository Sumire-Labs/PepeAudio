// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core.Enums;
using PepeAudio.Discord.Components;
using Xunit;

namespace PepeAudio.Discord.Tests;

public class PlayerRenderingTests
{
    [Fact]
    public void ProgressBar_shows_live_when_duration_unknown()
    {
        var s = ProgressBarRenderer.Render(1000, 0);
        Assert.Contains("LIVE", s);
    }

    [Fact]
    public void ProgressBar_places_knob_within_bounds()
    {
        var s = ProgressBarRenderer.Render(90_000, 180_000);
        Assert.Contains("🔘", s);
        Assert.Contains("1:30", s);
    }

    [Theory]
    [InlineData(0, "0:00")]
    [InlineData(65_000, "1:05")]
    [InlineData(3_725_000, "1:02:05")]
    public void Time_formats_minutes_and_hours(long ms, string expected)
        => Assert.Equal(expected, ProgressBarRenderer.Time(ms));

    [Fact]
    public void CustomId_roundtrips()
    {
        var id = PlayerCustomIds.For(PlayerControl.ToggleAura);
        var action = id.Split(':')[1];
        Assert.True(PlayerCustomIds.TryParse(action, out var parsed));
        Assert.Equal(PlayerControl.ToggleAura, parsed);
    }
}
