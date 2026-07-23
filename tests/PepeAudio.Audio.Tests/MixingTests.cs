// SPDX-License-Identifier: Apache-2.0
using System.Runtime.InteropServices;
using PepeAudio.Audio;
using Xunit;

namespace PepeAudio.Audio.Tests;

public class MixingTests
{
    [Fact]
    public void Crossfade_is_equal_power()
    {
        Assert.Equal((1.0, 0.0), Round(CrossfadeMixer.Gains(0)));
        Assert.Equal((0.0, 1.0), Round(CrossfadeMixer.Gains(1)));

        var (o, i) = CrossfadeMixer.Gains(0.5);
        Assert.Equal(1.0, o * o + i * i, 3); // constant power across the fade
    }

    [Fact]
    public void Mixer_sums_gain_scaled_inputs_with_master_volume()
    {
        var a = FrameOf(1000);
        var b = FrameOf(2000);
        var outBuf = new byte[PcmFormat.FrameBytes];

        PcmMixer.Mix(new List<(byte[], int, double)>
        {
            (a, a.Length, 1.0),
            (b, b.Length, 0.5),
        }, masterVolume: 100, outBuf);

        Assert.Equal(2000, First(outBuf)); // 1000*1.0 + 2000*0.5
    }

    [Fact]
    public void Mixer_clips_to_int16()
    {
        var loud = FrameOf(30000);
        var outBuf = new byte[PcmFormat.FrameBytes];
        PcmMixer.Mix(new List<(byte[], int, double)> { (loud, loud.Length, 1.0) }, masterVolume: 200, outBuf);
        Assert.Equal(short.MaxValue, First(outBuf));
    }

    private static (double, double) Round((double a, double b) g) => (Math.Round(g.a, 6), Math.Round(g.b, 6));

    private static byte[] FrameOf(short value)
    {
        var frame = new byte[PcmFormat.FrameBytes];
        MemoryMarshal.Cast<byte, short>(frame.AsSpan()).Fill(value);
        return frame;
    }

    private static short First(byte[] frame) => MemoryMarshal.Cast<byte, short>(frame)[0];
}
