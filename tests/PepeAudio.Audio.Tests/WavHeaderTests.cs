// SPDX-License-Identifier: Apache-2.0
using System.Buffers.Binary;
using PepeAudio.Audio.Effects;
using Xunit;

namespace PepeAudio.Audio.Tests;

public class WavHeaderTests
{
    [Theory]
    [InlineData(2)]
    [InlineData(4)]
    [InlineData(14)]
    public void Reads_channel_count(short channels)
    {
        var path = Path.GetTempFileName();
        try
        {
            File.WriteAllBytes(path, BuildWav(channels));
            Assert.Equal(channels, WavHeader.ReadChannels(path));
        }
        finally { File.Delete(path); }
    }

    [Fact]
    public void Returns_zero_for_non_wav()
    {
        var path = Path.GetTempFileName();
        try
        {
            File.WriteAllText(path, "not a wav file at all");
            Assert.Equal(0, WavHeader.ReadChannels(path));
        }
        finally { File.Delete(path); }
    }

    private static byte[] BuildWav(short channels)
    {
        var b = new byte[44];
        "RIFF"u8.CopyTo(b);
        BinaryPrimitives.WriteUInt32LittleEndian(b.AsSpan(4), 36);
        "WAVE"u8.CopyTo(b.AsSpan(8));
        "fmt "u8.CopyTo(b.AsSpan(12));
        BinaryPrimitives.WriteUInt32LittleEndian(b.AsSpan(16), 16); // fmt size
        BinaryPrimitives.WriteUInt16LittleEndian(b.AsSpan(20), 1);  // PCM
        BinaryPrimitives.WriteUInt16LittleEndian(b.AsSpan(22), (ushort)channels);
        BinaryPrimitives.WriteUInt32LittleEndian(b.AsSpan(24), 48000);
        "data"u8.CopyTo(b.AsSpan(36));
        return b;
    }
}
