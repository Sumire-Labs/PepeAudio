// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Audio;

// Equal-power crossfade envelope: avoids the mid-point loudness dip a linear
// fade produces. progress goes 0 -> 1 across the crossfade window.
public static class CrossfadeMixer
{
    public static (double OutGain, double InGain) Gains(double progress)
    {
        var p = Math.Clamp(progress, 0.0, 1.0);
        var angle = p * (Math.PI / 2);
        return (Math.Cos(angle), Math.Sin(angle));
    }
}
