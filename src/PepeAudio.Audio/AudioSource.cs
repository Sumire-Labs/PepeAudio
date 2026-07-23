// SPDX-License-Identifier: Apache-2.0
using Microsoft.Extensions.Logging;
using PepeAudio.Audio.Effects;
using PepeAudio.Core.Contracts;

namespace PepeAudio.Audio;

// One decoded track: FFmpeg (with effect chain) -> prebuffer. Reads 20ms frames.
public sealed class AudioSource : IDisposable
{
    private readonly PcmSourceBuffer _buffer;
    private long _framesRead;

    private AudioSource(PlayableRef track, long seekMs, PcmSourceBuffer buffer)
    {
        Track = track;
        SeekMs = seekMs;
        _buffer = buffer;
    }

    public PlayableRef Track { get; }
    public long SeekMs { get; }
    public long PositionMs => SeekMs + _framesRead * PcmFormat.FrameMs;

    public static async Task<AudioSource> StartAsync(
        AudioOptions opt, EffectChainBuilder chain, EffectSettings fx,
        IStreamProvider streams, PlayableRef track, long seekMs, ILogger log, CancellationToken ct)
    {
        var input = await streams.ResolveStreamAsync(track, ct);
        var http = input.StartsWith("http", StringComparison.OrdinalIgnoreCase);
        var ff = FFmpegProcess.Start(opt.FFmpegPath, input, http, seekMs, chain.Build(fx), log);
        var prebufferFrames = Math.Max(5, opt.PcmPrebufferMs / PcmFormat.FrameMs);
        return new AudioSource(track, seekMs, new PcmSourceBuffer(ff, prebufferFrames));
    }

    public async ValueTask<byte[]?> NextFrameAsync(CancellationToken ct)
    {
        var frame = await _buffer.ReadFrameAsync(ct);
        if (frame is null) return null;
        _framesRead++;
        return frame;
    }

    public void Cancel() => _buffer.Cancel();

    public void Dispose() => _buffer.Dispose();
}
