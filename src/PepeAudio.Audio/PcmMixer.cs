// SPDX-License-Identifier: Apache-2.0
using System.Runtime.InteropServices;

namespace PepeAudio.Audio;

// Mixes gain-scaled PCM frames into one output frame, applying master volume
// (0..200 -> /100) and clipping to int16.
public static class PcmMixer
{
    public static void Mix(IReadOnlyList<(byte[] Frame, int Length, double Gain)> inputs, int masterVolume, byte[] output)
    {
        Array.Clear(output);
        var master = masterVolume / 100.0;
        var outSamples = MemoryMarshal.Cast<byte, short>(output.AsSpan());

        foreach (var (frame, length, gain) in inputs)
        {
            var g = gain * master;
            if (g == 0) continue;
            var inSamples = MemoryMarshal.Cast<byte, short>(frame.AsSpan(0, length));
            var n = Math.Min(inSamples.Length, outSamples.Length);
            for (var i = 0; i < n; i++)
            {
                // Round, don't truncate: at fractional gains truncation-toward-zero adds a full
                // LSB of asymmetric quantization noise on top of the upstream dither.
                var mixed = outSamples[i] + (int)Math.Round(inSamples[i] * g);
                outSamples[i] = (short)Math.Clamp(mixed, short.MinValue, short.MaxValue);
            }
        }
    }
}
