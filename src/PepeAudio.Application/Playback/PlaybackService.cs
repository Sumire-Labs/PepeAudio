// SPDX-License-Identifier: Apache-2.0
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using PepeAudio.Audio;
using PepeAudio.Audio.Effects;
using PepeAudio.Cache;
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Enums;
using PepeAudio.Core.Exceptions;
using PepeAudio.Core.Observability;
using PepeAudio.Core.Sharding;
using PepeAudio.Data.Repositories;
using PepeAudio.Sources;
using PepeAudio.Sources.Models;
using PepeAudio.Sources.YtDlp;

namespace PepeAudio.Application.Playback;

public interface IPlaybackService
{
    Task<PlayResult> PlayAsync(PlayRequest req, CancellationToken ct);
    Task QuitAsync(ulong guildId);
    Task ControlAsync(ControlEnvelope envelope);
    void ApplyLocal(ControlEnvelope envelope);
    Task<bool> ToggleAutoplayAsync(ulong guildId);
    Task<bool> SetAutoplayAsync(ulong guildId, bool enabled);
    Task<IReadOnlyList<SearchCandidate>> SearchAsync(string query, CancellationToken ct);
    Task<IReadOnlyList<TrackInfo>> ResolveInfoAsync(string input, ulong requesterId, int max, CancellationToken ct);
    PlayerState GetState(ulong guildId);
    IReadOnlyList<string> PresetNames { get; }
}

public sealed class PlaybackService : IPlaybackService
{
    private static readonly PlayerControl[] Persisted =
        { PlayerControl.VolumeUp, PlayerControl.VolumeDown, PlayerControl.SetVolume, PlayerControl.ToggleAura, PlayerControl.ToggleAura360, PlayerControl.SetPreset };

    private const int SearchResultCount = 8;

    private readonly IResolverRegistry _resolver;
    private readonly IYtDlpClient _ytdlp;
    private readonly IPlayerManager _players;
    private readonly IGuildSettingsRepository _settings;
    private readonly IShardTopology _topology;
    private readonly ICommandBus _bus;
    private readonly ShutdownState _shutdown;
    private readonly int _maxVoices;
    private readonly IReadOnlyList<string> _presetNames;
    private readonly ILogger<PlaybackService> _log;

    public PlaybackService(IResolverRegistry resolver, IYtDlpClient ytdlp, IPlayerManager players, IGuildSettingsRepository settings,
        IShardTopology topology, ICommandBus bus, ShutdownState shutdown, IOptions<AudioOptions> audio,
        IHeSuViPresetLibrary presets, ILogger<PlaybackService> log)
    {
        _resolver = resolver;
        _ytdlp = ytdlp;
        _players = players;
        _settings = settings;
        _topology = topology;
        _bus = bus;
        _shutdown = shutdown;
        _maxVoices = audio.Value.MaxConcurrentVoices;
        _presetNames = presets.All.Select(p => p.Name).ToList();
        _log = log;
    }

    // yt-dlp search for the web "add track" panel. Candidates carry no thumbnail, so we
    // synthesise the YouTube still from the video id (search is always ytsearch:).
    public async Task<IReadOnlyList<SearchCandidate>> SearchAsync(string query, CancellationToken ct)
    {
        var results = await _ytdlp.SearchAsync(query, SearchResultCount, ct);
        return results.Select(c => new SearchCandidate(
            c.Title, c.Channel ?? string.Empty, c.WebpageUrl,
            $"https://i.ytimg.com/vi/{c.Id}/mqdefault.jpg")).ToList();
    }

    // Resolves a URL/search term to track metadata without enqueueing — used by playlist import.
    // Bounded by max so a huge collection URL can't balloon the response.
    public async Task<IReadOnlyList<TrackInfo>> ResolveInfoAsync(string input, ulong requesterId, int max, CancellationToken ct)
    {
        var request = new ResolveRequest(input, null, requesterId);
        var list = new List<TrackInfo>();
        await foreach (var track in _resolver.ResolveAsync(request, ct))
        {
            list.Add(track.Info);
            if (list.Count >= max) break;
        }
        return list;
    }

    public IReadOnlyList<string> PresetNames => _presetNames;

    public async Task<PlayResult> PlayAsync(PlayRequest req, CancellationToken ct)
    {
        using var activity = PepeMetrics.Trace.StartActivity("play");
        activity?.SetTag("guild.id", req.GuildId.ToString());

        if (_shutdown.IsDraining)
            throw new PepeAudioException("BOT を再起動中です。しばらくしてからもう一度お試しください。");
        if (!_players.TryGet(req.GuildId, out _) && _players.ActiveCount >= _maxVoices)
            throw new CapacityExceededException("現在このホストは処理上限に達しています。しばらくしてからお試しください。");

        var player = _players.GetOrCreate(req.GuildId);
        player.ApplySettings(await _settings.GetAsync(req.GuildId, ct));
        await player.EnsureConnectedAsync(req.VoiceChannelId, ct);

        var request = new ResolveRequest(
            req.Url,
            req.File is null ? null : new AttachmentRef(req.File.Url, req.File.FileName, req.File.ContentType, req.File.Size),
            req.RequesterId);

        TrackInfo? first = null;
        var count = 0;
        await foreach (var track in _resolver.ResolveAsync(request, ct))
        {
            player.Enqueue(track);
            first ??= track.Info;
            count++;
        }

        if (first is null)
        {
            PepeMetrics.ResolveFailures.Add(1);
            throw new ResolveFailedException("入力から再生可能なトラックが見つかりませんでした。");
        }

        PepeMetrics.TracksEnqueued.Add(count);
        _log.LogInformation("Enqueued {Count} track(s) in guild {Guild}", count, req.GuildId);
        return new PlayResult(first, count);
    }

    public Task QuitAsync(ulong guildId) => _players.RemoveAsync(guildId);

    // Flips the persisted autoplay flag and pushes it to the live player (owner shard aware).
    public async Task<bool> ToggleAutoplayAsync(ulong guildId)
    {
        var settings = await _settings.GetAsync(guildId, CancellationToken.None);
        return await SetAutoplayAsync(guildId, !settings.Autoplay);
    }

    // Sets autoplay to an explicit value (web toggle), persists it, and pushes to the live player.
    public async Task<bool> SetAutoplayAsync(ulong guildId, bool enabled)
    {
        var settings = await _settings.GetAsync(guildId, CancellationToken.None);
        settings.Autoplay = enabled;
        await _settings.UpsertAsync(settings, CancellationToken.None);
        await ControlAsync(new ControlEnvelope(guildId, PlayerControl.SetAutoplay,
            enabled ? "1" : "0", 0, 0, DateTimeOffset.UtcNow));
        return enabled;
    }

    // Routes control to the process that owns the guild's shard (unified chain).
    public async Task ControlAsync(ControlEnvelope envelope)
    {
        using var activity = PepeMetrics.Trace.StartActivity("control");
        activity?.SetTag("control", envelope.Control.ToString());
        if (_topology.Owns(envelope.GuildId))
            ApplyLocal(envelope);
        else
            await _bus.PublishAsync(_topology.OwnerShardFor(envelope.GuildId), envelope);
    }

    public void ApplyLocal(ControlEnvelope envelope)
    {
        // Stop is a definitive stop: evict the player so its one-shot teardown never
        // leaves a dead instance behind that a later /play would reuse (and never play).
        if (envelope.Control == PlayerControl.Stop)
        {
            _ = _players.RemoveAsync(envelope.GuildId);
            return;
        }
        if (!_players.TryGet(envelope.GuildId, out var player) || player is null)
            return;
        player.Apply(envelope);
        PepeMetrics.ControlCommands.Add(1);
        if (Persisted.Contains(envelope.Control))
            _ = PersistAsync(envelope.GuildId, player);
    }

    public PlayerState GetState(ulong guildId)
        => _players.TryGet(guildId, out var player) && player is not null
            ? player.Snapshot()
            : PlayerState.Empty(guildId);

    private async Task PersistAsync(ulong guildId, IGuildPlayer player)
    {
        try
        {
            var settings = await _settings.GetAsync(guildId, CancellationToken.None);
            var fx = player.CurrentSettings;
            settings.AuraEnabled = fx.AuraEnabled;
            settings.Aura360Enabled = fx.Aura360Enabled;
            settings.PresetName = fx.PresetName;
            settings.Volume = fx.Volume;
            settings.Normalization = fx.Normalization;
            settings.CrossfadeMs = fx.CrossfadeMs;
            await _settings.UpsertAsync(settings, CancellationToken.None);
        }
        catch (Exception ex)
        {
            _log.LogDebug(ex, "Persisting audio settings skipped for {Guild}", guildId);
        }
    }
}
