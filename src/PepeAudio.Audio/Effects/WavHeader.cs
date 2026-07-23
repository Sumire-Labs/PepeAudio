// SPDX-License-Identifier: Apache-2.0
using System.Buffers.Binary;

namespace PepeAudio.Audio.Effects;

// Minimal WAV header reader: returns the channel count from the "fmt " chunk.
public static class WavHeader
{
    public static int ReadChannels(string path)
    {
        try
        {
            using var fs = File.OpenRead(path);
            Span<byte> head = stackalloc byte[12];
            if (fs.Read(head) != 12) return 0;
            if (head[0] != 'R' || head[1] != 'I' || head[2] != 'F' || head[3] != 'F') return 0;
            if (head[8] != 'W' || head[9] != 'A' || head[10] != 'V' || head[11] != 'E') return 0;

            Span<byte> chunk = stackalloc byte[8];
            while (fs.Read(chunk) == 8)
            {
                var id = System.Text.Encoding.ASCII.GetString(chunk[..4]);
                var size = BinaryPrimitives.ReadUInt32LittleEndian(chunk[4..]);
                if (id == "fmt ")
                {
                    Span<byte> fmt = stackalloc byte[4];
                    if (fs.Read(fmt) != 4) return 0;
                    return BinaryPrimitives.ReadUInt16LittleEndian(fmt[2..]); // numChannels
                }
                fs.Seek(size + (size & 1), SeekOrigin.Current);
            }
            return 0;
        }
        catch
        {
            return 0;
        }
    }
}
