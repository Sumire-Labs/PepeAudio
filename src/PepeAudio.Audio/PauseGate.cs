// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Audio;

// Lets the stream loop block while playback is paused (FFmpeg back-pressures on its pipe).
public sealed class PauseGate
{
    private readonly ManualResetEventSlim _gate = new(initialState: true);

    public bool IsPaused => !_gate.IsSet;
    public void Toggle() { if (_gate.IsSet) _gate.Reset(); else _gate.Set(); }

    public Task WaitAsync(CancellationToken ct)
    {
        if (_gate.IsSet) return Task.CompletedTask;
        return Task.Run(() => _gate.Wait(ct), ct);
    }
}
