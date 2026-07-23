// SPDX-License-Identifier: Apache-2.0
using Discord.Audio;
using Discord.WebSocket;
using Microsoft.Extensions.Logging;
using PepeAudio.Audio.Effects;
using PepeAudio.Cache;
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Enums;
using PepeAudio.Core.Models;

namespace PepeAudio.Audio;

public sealed class GuildPlayer : IGuildPlayer
{
    private const int PrefetchMs = 1500;
    private const int EffectChangeFrames = 3; // ~60ms crossfade to hide a respawn seam
    private const int ReconnectAttempts = 5;
    private const int HistoryMax = 50;
    private const int AutoplayBatch = 10;
    private const int MirrorIntervalFrames = 50;    // ~1s at 20ms/frame: mirror state + renew voice lock
    private const int CheckpointEveryMirrors = 15;  // ~15s between durable checkpoints
    private const int MaxConsecutiveFailures = 5;   // stop after this many unplayable tracks in a row
    private const int ReconnectDelayMs = 1000;
    private const int MaxVolume = 200;
    private const int VolumeStep = 10;
    private static readonly TimeSpan VoiceLockTtl = TimeSpan.FromSeconds(30);
    private static readonly TimeSpan IdleTimeout = TimeSpan.FromMinutes(5);

    private readonly DiscordShardedClient _client;
    private readonly AudioOptions _opt;
    private readonly EffectChainBuilder _chain;
    private readonly IStreamProvider _streams;
    private readonly IAutoplayProvider _autoplay;
    private readonly IValkeyLock _lock;
    private readonly ICheckpointStore _checkpoints;
    private readonly IPlayerStateStore _store;
    private readonly ILogger _log;
    private readonly Func<ulong, Task> _terminate;
    private readonly string _voiceToken = Guid.NewGuid().ToString("N");
    private readonly string _voiceKey;
    private long _resumeSeekMs;
    private int _checkpointTick;

    private readonly object _sync = new();
    private readonly List<PlayableRef> _queue = new();
    private readonly List<PlayableRef> _history = new();
    private readonly PauseGate _pause = new();
    private readonly CancellationTokenSource _lifetime = new();
    private readonly byte[] _out = new byte[PcmFormat.FrameBytes];
    private CancellationTokenSource? _idleCts;

    private IAudioClient? _audio;
    private AudioOutStream? _pcm;
    private ulong _voiceChannelId;
    private Task? _pump;

    private AudioSource? _primary;
    private AudioSource? _incoming;
    private PlayableRef? _lastTrack;
    private bool _crossfading;
    private int _crossfadeFrames;
    private int _crossfadeElapsed;
    private bool _pendingEffect;
    private bool _skip;
    private int _mirrorTick;

    private EffectSettings _fx = new();
    private LoopMode _loopMode = LoopMode.Off;
    private bool _shuffle;
    private bool _autoplayEnabled;
    private bool _navBack;
    private long _epoch;

    public GuildPlayer(ulong guildId, DiscordShardedClient client, AudioOptions opt, EffectChainBuilder chain,
        IStreamProvider streams, IAutoplayProvider autoplay, IValkeyLock voiceLock, ICheckpointStore checkpoints,
        IPlayerStateStore store, ILogger log, Func<ulong, Task> terminate)
    {
        GuildId = guildId;
        _client = client;
        _opt = opt;
        _chain = chain;
        _streams = streams;
        _autoplay = autoplay;
        _lock = voiceLock;
        _checkpoints = checkpoints;
        _store = store;
        _log = log;
        _terminate = terminate;
        _voiceKey = ValkeyKeys.VoiceLock(guildId);
    }

    public ulong GuildId { get; }
    public ulong VoiceChannelId => _voiceChannelId;
    public EffectSettings CurrentSettings => _fx;

    // The bot was dragged to another channel; the voice connection already followed, so just
    // track the new id (occupancy checks and reconnect target must point at the live channel).
    public void SetVoiceChannel(ulong channelId) => _voiceChannelId = channelId;

    public void ApplySettings(GuildSettings settings)
    {
        var next = EffectSettings.From(settings);
        var effectChanged = _primary is not null &&
            (next.AuraEnabled != _fx.AuraEnabled || next.PresetName != _fx.PresetName ||
             next.Normalization != _fx.Normalization);
        _fx = next;
        _autoplayEnabled = settings.Autoplay;
        if (effectChanged) _pendingEffect = true;
    }

    public async Task EnsureConnectedAsync(ulong voiceChannelId, CancellationToken ct)
    {
        if (_audio is { ConnectionState: Discord.ConnectionState.Connected } && _voiceChannelId == voiceChannelId)
            return;

        var owned = await _lock.TryAcquireAsync(_voiceKey, _voiceToken, VoiceLockTtl)
            || await _lock.RenewAsync(_voiceKey, _voiceToken, VoiceLockTtl);
        if (!owned)
            throw new InvalidOperationException("このサーバーでは既に別のインスタンスが再生中です。");

        var channel = _client.GetGuild(GuildId)?.GetVoiceChannel(voiceChannelId)
            ?? throw new InvalidOperationException("ボイスチャンネルが見つかりません。");
        _audio = await channel.ConnectAsync();
        _pcm = _audio.CreatePCMStream(AudioApplication.Music, channel.Bitrate, bufferMillis: _opt.DiscordPcmBufferMs);
        _voiceChannelId = voiceChannelId;
        _epoch++;
    }

    public void Enqueue(PlayableRef track)
    {
        CancelIdleTimer();
        lock (_sync)
        {
            _queue.Add(track);
            StartPumpIfIdle();
        }
    }

    // Check-and-start under _sync so concurrent Enqueue/Restore never launch two pumps.
    private void StartPumpIfIdle()
    {
        if (_pump is null || _pump.IsCompleted) _pump = Task.Run(RunPumpAsync);
    }

    private async Task RunPumpAsync()
    {
        var ct = _lifetime.Token;
        var dropped = false;
        try
        {
            while (!ct.IsCancellationRequested)
            {
                _primary ??= await StartNextAsync(ct);
                if (_primary is null) break;

                var pf = await _primary.NextFrameAsync(ct);
                if (pf is null)
                {
                    var finished = _primary.Track;
                    _primary.Dispose();
                    _primary = _incoming;
                    _incoming = null;
                    _crossfading = false;
                    _skip = false;
                    if (_navBack) _navBack = false; else PushHistory(finished);
                    if (_primary is not null) _lastTrack = _primary.Track;
                    _primary ??= await StartNextAsync(ct);
                    if (_primary is null) break;
                    continue;
                }

                if (!_crossfading && _incoming is null) await MaybeStartOverlapAsync(ct);

                double primaryGain = 1.0, inGain = 0;
                byte[]? inf = null;
                if (_crossfading && _incoming is not null)
                {
                    (primaryGain, inGain) = CrossfadeMixer.Gains((double)_crossfadeElapsed / _crossfadeFrames);
                    inf = await _incoming.NextFrameAsync(ct);
                    _crossfadeElapsed++;
                }

                var inputs = new List<(byte[], int, double)> { (pf, PcmFormat.FrameBytes, primaryGain) };
                if (inf is not null) inputs.Add((inf, PcmFormat.FrameBytes, inGain));

                await _pause.WaitAsync(ct);
                PcmMixer.Mix(inputs, _fx.Volume, _out);
                if (!await WriteWithRecoveryAsync(_out, ct)) { dropped = true; break; }

                if (_crossfading && _crossfadeElapsed >= _crossfadeFrames) Promote();
                if (++_mirrorTick >= MirrorIntervalFrames)
                {
                    _mirrorTick = 0;
                    await MirrorAsync();
                    await _lock.RenewAsync(_voiceKey, _voiceToken, VoiceLockTtl);
                    if (++_checkpointTick >= CheckpointEveryMirrors) { _checkpointTick = 0; await SaveCheckpointAsync(); }
                }
            }
        }
        catch (OperationCanceledException) { /* stopping */ }
        catch (Exception ex) { _log.LogWarning(ex, "Pump error in guild {Guild}", GuildId); }
        finally
        {
            _primary?.Dispose();
            _incoming?.Dispose();
            _primary = _incoming = null;
            await MirrorAsync();
            // Unrecoverable voice drop -> self-evict so the lock is released and no zombie lingers.
            // Queue drained cleanly -> leave the channel after a grace period.
            if (dropped) _ = _terminate(GuildId);
            else if (!ct.IsCancellationRequested) ArmIdleTimer();
        }
    }

    // Writes one frame, transparently rebuilding a dropped voice connection so playback resumes
    // the current track in place. Returns false only when reconnection is impossible.
    private async Task<bool> WriteWithRecoveryAsync(byte[] frame, CancellationToken ct)
    {
        try { await _pcm!.WriteAsync(frame, ct); return true; }
        catch (OperationCanceledException) { throw; }
        catch (Exception ex)
        {
            _log.LogWarning(ex, "Voice write failed in guild {Guild}; attempting reconnect", GuildId);
            return await ReconnectAsync(ct);
        }
    }

    private async Task<bool> ReconnectAsync(CancellationToken ct)
    {
        for (var attempt = 0; attempt < ReconnectAttempts && !ct.IsCancellationRequested; attempt++)
        {
            try
            {
                var channel = _client.GetGuild(GuildId)?.GetVoiceChannel(_voiceChannelId);
                if (channel is null) return false;
                if (_audio is not { ConnectionState: Discord.ConnectionState.Connected })
                {
                    try { if (_audio is not null) await _audio.StopAsync(); } catch { /* ignore */ }
                    _audio = await channel.ConnectAsync();
                }
                _pcm = _audio.CreatePCMStream(AudioApplication.Music, channel.Bitrate, bufferMillis: _opt.DiscordPcmBufferMs);
                _log.LogInformation("Voice reconnected in guild {Guild}", GuildId);
                return true;
            }
            catch { try { await Task.Delay(ReconnectDelayMs, ct); } catch { return false; } }
        }
        return false;
    }

    private void ArmIdleTimer()
    {
        CancelIdleTimer();
        var cts = new CancellationTokenSource();
        _idleCts = cts;
        _ = Task.Run(async () =>
        {
            try { await Task.Delay(IdleTimeout, cts.Token); }
            catch { return; }
            _log.LogInformation("Idle for {Min} min in guild {Guild}; leaving", IdleTimeout.TotalMinutes, GuildId);
            try { await _terminate(GuildId); } catch { /* already gone */ }
        });
    }

    private void CancelIdleTimer()
    {
        _idleCts?.Cancel();
        _idleCts = null;
    }

    private void Promote()
    {
        if (_primary is not null) PushHistory(_primary.Track);
        _primary?.Dispose();
        _primary = _incoming;
        _incoming = null;
        _crossfading = false;
        if (_primary is not null) _lastTrack = _primary.Track;
    }

    private void PushHistory(PlayableRef track)
    {
        lock (_sync)
        {
            _history.Add(track);
            if (_history.Count > HistoryMax) _history.RemoveAt(0);
        }
    }

    // Re-queues the current track and jumps back to the most recently finished one; repeated
    // presses walk back through history. A no-op when nothing has played yet.
    private void Previous()
    {
        lock (_sync)
        {
            if (_history.Count == 0) return;
            var prev = _history[^1];
            _history.RemoveAt(_history.Count - 1);
            var current = _primary?.Track;
            if (current is not null) { _queue.Insert(0, current); _navBack = true; }
            _queue.Insert(0, prev);
            _lastTrack = null; // don't let LoopMode.Track re-pin the interrupted track
            StartPumpIfIdle();
        }
        CancelIdleTimer();
        _primary?.Cancel();
    }

    // Queue drained with autoplay on: pull related tracks from the seed, skipping anything
    // already played this session so radio doesn't loop on the same handful.
    private async Task FillAutoplayAsync(CancellationToken ct)
    {
        var seed = _lastTrack;
        if (seed is null) return;
        try
        {
            var related = await _autoplay.RelatedAsync(seed, AutoplayBatch, ct);
            if (related.Count == 0) return;
            lock (_sync)
            {
                var seen = _history.Select(h => h.Info.Url).ToHashSet(StringComparer.OrdinalIgnoreCase);
                var fresh = related.Where(r => seen.Add(r.Info.Url)).ToList();
                _queue.AddRange(fresh.Count > 0 ? fresh : related);
            }
            _log.LogInformation("Autoplay queued related tracks in guild {Guild}", GuildId);
        }
        catch (OperationCanceledException) { throw; }
        catch (Exception ex) { _log.LogWarning(ex, "Autoplay failed in guild {Guild}", GuildId); }
    }

    private async Task MaybeStartOverlapAsync(CancellationToken ct)
    {
        if (_pendingEffect) { await StartEffectChangeAsync(ct); return; }
        if (_loopMode == LoopMode.Track) return;

        var dur = _primary!.Track.Info.DurationMs;
        if (dur <= 0) return;
        var remaining = dur - _primary.PositionMs;

        if (_fx.CrossfadeMs > 0 && remaining <= _fx.CrossfadeMs)
        {
            var next = DequeueForOverlap();
            if (next is null) return;
            var src = await TryStartAsync(next, 0, ct);
            if (src is null) return;
            _incoming = src;
            _crossfadeFrames = Math.Max(1, Math.Min(_fx.CrossfadeMs, (int)remaining) / PcmFormat.FrameMs);
            _crossfadeElapsed = 0;
            _crossfading = true;
        }
        else if (_fx.CrossfadeMs == 0 && remaining <= PrefetchMs)
        {
            var next = DequeueForOverlap();
            if (next is not null) _incoming = await TryStartAsync(next, 0, ct);
        }
    }

    private async Task StartEffectChangeAsync(CancellationToken ct)
    {
        _pendingEffect = false;
        if (_primary is null) return;
        var src = await TryStartAsync(_primary.Track, _primary.PositionMs, ct);
        if (src is null) return;
        _incoming = src;
        _crossfadeFrames = EffectChangeFrames;
        _crossfadeElapsed = 0;
        _crossfading = true;
    }

    private async Task<AudioSource?> StartNextAsync(CancellationToken ct)
    {
        var failures = 0;
        var autoplayTried = false;
        while (!ct.IsCancellationRequested)
        {
            PlayableRef? track;
            if (_loopMode == LoopMode.Track && _lastTrack is not null && !_skip)
                track = _lastTrack;
            else
                track = DequeueForOverlap();
            _skip = false;
            if (track is null)
            {
                if (!_autoplayEnabled || autoplayTried || _lastTrack is null) return null;
                autoplayTried = true;
                await FillAutoplayAsync(ct);
                continue;
            }
            var seek = _resumeSeekMs;
            _resumeSeekMs = 0;
            var source = await TryStartAsync(track, seek, ct);
            if (source is not null) { _lastTrack = track; return source; }
            _lastTrack = null; // a failing track must not pin LoopMode.Track
            if (++failures >= MaxConsecutiveFailures) return null;
        }
        return null;
    }

    // Starts one track, isolating a resolve/ffmpeg failure so a single bad track is
    // skipped instead of tearing down the whole pump. Cancellation still propagates.
    private async Task<AudioSource?> TryStartAsync(PlayableRef track, long seekMs, CancellationToken ct)
    {
        try
        {
            return await AudioSource.StartAsync(_opt, _chain, _fx, _streams, track, seekMs, _log, ct);
        }
        catch (OperationCanceledException) { throw; }
        catch (Exception ex)
        {
            _log.LogWarning(ex, "Skipping unplayable track '{Title}' in guild {Guild}", track.Info.Title, GuildId);
            return null;
        }
    }

    private PlayableRef? DequeueForOverlap()
    {
        lock (_sync)
        {
            if (_queue.Count == 0) return null;
            var i = _shuffle ? Random.Shared.Next(_queue.Count) : 0;
            var track = _queue[i];
            _queue.RemoveAt(i);
            if (_loopMode == LoopMode.Queue) _queue.Add(track);
            return track;
        }
    }

    public void Apply(ControlEnvelope envelope)
    {
        switch (envelope.Control)
        {
            case PlayerControl.PlayPause: _pause.Toggle(); break;
            case PlayerControl.Skip: _skip = true; _primary?.Cancel(); break;
            case PlayerControl.Stop: _ = StopAsync(); break;
            case PlayerControl.VolumeUp: _fx.Volume = Math.Min(MaxVolume, _fx.Volume + VolumeStep); break;
            case PlayerControl.VolumeDown: _fx.Volume = Math.Max(0, _fx.Volume - VolumeStep); break;
            case PlayerControl.SetVolume:
                if (int.TryParse(envelope.Arg, out var vol)) _fx.Volume = Math.Clamp(vol, 0, MaxVolume);
                break;
            case PlayerControl.Loop: _loopMode = (LoopMode)(((int)_loopMode + 1) % 3); break;
            case PlayerControl.Shuffle: _shuffle = !_shuffle; break;
            case PlayerControl.ToggleAura: _fx.AuraEnabled = !_fx.AuraEnabled; _pendingEffect = true; break;
            case PlayerControl.SetPreset:
                if (!string.IsNullOrWhiteSpace(envelope.Arg)) { _fx.PresetName = envelope.Arg; _pendingEffect = true; }
                break;
            case PlayerControl.SetAutoplay: _autoplayEnabled = envelope.Arg == "1"; break;
            case PlayerControl.ReorderQueue: Reorder(envelope.Arg); break;
            case PlayerControl.RemoveTrack: RemoveAt(envelope.Arg); break;
            case PlayerControl.Previous: Previous(); break;
            case PlayerControl.ShowQueue: break;
        }
        _ = MirrorAsync();
    }

    private void Reorder(string? arg)
    {
        var parts = arg?.Split(':');
        if (parts is not { Length: 2 } || !int.TryParse(parts[0], out var from) || !int.TryParse(parts[1], out var to))
            return;
        lock (_sync)
        {
            if (from < 0 || from >= _queue.Count || to < 0 || to >= _queue.Count) return;
            var item = _queue[from];
            _queue.RemoveAt(from);
            _queue.Insert(to, item);
        }
    }

    private void RemoveAt(string? arg)
    {
        if (!int.TryParse(arg, out var index)) return;
        lock (_sync)
            if (index >= 0 && index < _queue.Count) _queue.RemoveAt(index);
    }

    // Restores queue + position from a durable checkpoint and reconnects voice.
    public async Task RestoreAsync(PlayerCheckpoint checkpoint, CancellationToken ct)
    {
        CancelIdleTimer();
        _loopMode = checkpoint.Loop;
        _shuffle = checkpoint.Shuffle;
        lock (_sync)
        {
            _queue.Clear();
            if (checkpoint.Current is not null) _queue.Add(checkpoint.Current);
            _queue.AddRange(checkpoint.Queue);
        }
        _resumeSeekMs = checkpoint.Current is not null ? checkpoint.PositionMs : 0;
        await EnsureConnectedAsync(checkpoint.VoiceChannelId, ct);
        lock (_sync) StartPumpIfIdle();
    }

    public async Task DrainAsync()
    {
        await SaveCheckpointAsync();
        await TeardownAsync();
    }

    public async Task StopAsync()
    {
        await TeardownAsync();
        await _checkpoints.DeleteAsync(GuildId, CancellationToken.None);
    }

    private async Task TeardownAsync()
    {
        CancelIdleTimer();
        lock (_sync) _queue.Clear();
        _lifetime.Cancel();
        _primary?.Cancel();
        _incoming?.Cancel();
        if (_audio is not null) { try { await _audio.StopAsync(); } catch { /* ignore */ } }
        await _lock.ReleaseAsync(_voiceKey, _voiceToken);
        await MirrorAsync();
    }

    private async Task SaveCheckpointAsync()
    {
        try
        {
            List<PlayableRef> queue;
            lock (_sync) queue = _queue.ToList();
            if (_primary is null && queue.Count == 0)
            {
                await _checkpoints.DeleteAsync(GuildId, CancellationToken.None);
                return;
            }
            var checkpoint = new PlayerCheckpoint(GuildId, _voiceChannelId, _primary?.PositionMs ?? 0,
                _primary?.Track, queue, _loopMode, _shuffle);
            await _checkpoints.SaveAsync(checkpoint, CancellationToken.None);
        }
        catch { /* best effort */ }
    }

    public PlayerState Snapshot()
    {
        List<QueueEntry> entries;
        lock (_sync) entries = _queue.Select((t, i) => new QueueEntry(i, t.Info)).ToList();

        return new PlayerState(
            GuildId, _primary?.Track.Info, _primary?.PositionMs ?? 0,
            _primary is not null && !_pause.IsPaused, _fx.Volume, _loopMode, _shuffle,
            _fx.AuraEnabled, _fx.PresetName, _fx.CrossfadeMs, entries, _epoch, DateTimeOffset.UtcNow);
    }

    private async Task MirrorAsync()
    {
        try { await _store.SetAsync(Snapshot()); } catch { /* best effort */ }
    }
}
