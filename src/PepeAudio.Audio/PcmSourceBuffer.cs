// SPDX-License-Identifier: Apache-2.0
using System.Threading.Channels;

namespace PepeAudio.Audio;

// Drains FFmpeg stdout into fixed-size PCM frames on a background task. The
// bounded channel provides the fast-start prebuffer and jitter tolerance;
// FFmpeg back-pressures on its pipe when the channel is full.
public sealed class PcmSourceBuffer : IDisposable
{
    private readonly FFmpegProcess _ff;
    private readonly Channel<byte[]> _channel;
    private readonly CancellationTokenSource _cts = new();

    public PcmSourceBuffer(FFmpegProcess ff, int prebufferFrames)
    {
        _ff = ff;
        _channel = Channel.CreateBounded<byte[]>(new BoundedChannelOptions(Math.Max(2, prebufferFrames))
        {
            FullMode = BoundedChannelFullMode.Wait,
            SingleReader = true,
            SingleWriter = true,
        });
        _ = Task.Run(FillAsync);
    }

    private async Task FillAsync()
    {
        try
        {
            var stream = _ff.Output;
            while (!_cts.IsCancellationRequested)
            {
                var frame = new byte[PcmFormat.FrameBytes];
                var read = await ReadFullAsync(stream, frame, _cts.Token);
                if (read == 0) break;
                if (read < frame.Length) Array.Clear(frame, read, frame.Length - read);
                await _channel.Writer.WriteAsync(frame, _cts.Token);
            }
        }
        catch (OperationCanceledException) { /* stopping */ }
        catch { /* ffmpeg died */ }
        finally { _channel.Writer.TryComplete(); }
    }

    public async ValueTask<byte[]?> ReadFrameAsync(CancellationToken ct)
    {
        try { return await _channel.Reader.ReadAsync(ct); }
        catch (ChannelClosedException) { return null; }
        catch (OperationCanceledException) { return null; }
    }

    public void Cancel() { try { _cts.Cancel(); } catch (ObjectDisposedException) { /* already torn down */ } }

    private static async ValueTask<int> ReadFullAsync(Stream s, byte[] buffer, CancellationToken ct)
    {
        var total = 0;
        while (total < buffer.Length)
        {
            var n = await s.ReadAsync(buffer.AsMemory(total), ct);
            if (n == 0) break;
            total += n;
        }
        return total;
    }

    public void Dispose()
    {
        try { _cts.Cancel(); } catch (ObjectDisposedException) { }
        _ff.Dispose();
        _cts.Dispose();
    }
}
