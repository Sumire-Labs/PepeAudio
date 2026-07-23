// SPDX-License-Identifier: Apache-2.0
using Dapper;
using Microsoft.Extensions.Logging;
using PepeAudio.Core.Enums;
using PepeAudio.Core.Models;

namespace PepeAudio.Data.Repositories;

public interface IGuildSettingsRepository
{
    Task<GuildSettings> GetAsync(ulong guildId, CancellationToken ct);
    Task UpsertAsync(GuildSettings settings, CancellationToken ct);
}

public sealed class GuildSettingsRepository : IGuildSettingsRepository
{
    private readonly INpgsqlDataSourceProvider _db;
    private readonly ILogger<GuildSettingsRepository> _log;

    public GuildSettingsRepository(INpgsqlDataSourceProvider db, ILogger<GuildSettingsRepository> log)
    {
        _db = db;
        _log = log;
    }

    public async Task<GuildSettings> GetAsync(ulong guildId, CancellationToken ct)
    {
        var src = _db.DataSource;
        if (src is null) return GuildSettings.Default(guildId);
        try
        {
            await using var conn = await src.OpenConnectionAsync(ct);
            var row = await conn.QuerySingleOrDefaultAsync<Row>(new CommandDefinition(
                "SELECT guild_id, aura_enabled, preset_name, volume, normalization, crossfade_ms, dj_role_id, autoplay, bound_text_channel_id, locale " +
                "FROM guild_settings WHERE guild_id = @guildId",
                new { guildId = (long)guildId }, cancellationToken: ct));
            return row is null ? GuildSettings.Default(guildId) : row.ToModel();
        }
        catch (Exception ex) when (ex is not OperationCanceledException)
        {
            _log.LogDebug(ex, "Guild settings read failed for {Guild}; using defaults", guildId);
            return GuildSettings.Default(guildId);
        }
    }

    public async Task UpsertAsync(GuildSettings s, CancellationToken ct)
    {
        var src = _db.DataSource;
        if (src is null) return;
        try
        {
            await using var conn = await src.OpenConnectionAsync(ct);
            await conn.ExecuteAsync(new CommandDefinition(
            "INSERT INTO guilds(guild_id) VALUES (@gid) ON CONFLICT DO NOTHING;" +
            "INSERT INTO guild_settings(guild_id, aura_enabled, preset_name, volume, normalization, crossfade_ms, dj_role_id, autoplay, bound_text_channel_id, locale, updated_at) " +
            "VALUES (@gid, @aura, @preset, @vol, @norm, @cf, @dj, @autoplay, @bound, @locale, now()) " +
            "ON CONFLICT (guild_id) DO UPDATE SET aura_enabled=@aura, preset_name=@preset, volume=@vol, normalization=@norm, " +
            "crossfade_ms=@cf, dj_role_id=@dj, autoplay=@autoplay, bound_text_channel_id=@bound, locale=@locale, updated_at=now()",
            new
            {
                gid = (long)s.GuildId,
                aura = s.AuraEnabled,
                preset = s.PresetName,
                vol = (short)s.Volume,
                norm = s.Normalization.ToString(),
                cf = (short)s.CrossfadeMs,
                dj = (long?)s.DjRoleId,
                autoplay = s.Autoplay,
                bound = (long?)s.BoundTextChannelId,
                locale = s.Locale,
            }, cancellationToken: ct));
        }
        catch (Exception ex) when (ex is not OperationCanceledException)
        {
            _log.LogDebug(ex, "Guild settings upsert failed for {Guild}", s.GuildId);
        }
    }

    private sealed class Row
    {
        public long guild_id { get; set; }
        public bool aura_enabled { get; set; }
        public string preset_name { get; set; } = "Aura";
        public short volume { get; set; }
        public string normalization { get; set; } = "Off";
        public short crossfade_ms { get; set; }
        public long? dj_role_id { get; set; }
        public bool autoplay { get; set; }
        public long? bound_text_channel_id { get; set; }
        public string locale { get; set; } = "en-US";

        public GuildSettings ToModel() => new()
        {
            GuildId = (ulong)guild_id,
            AuraEnabled = aura_enabled,
            PresetName = preset_name,
            Volume = volume,
            Normalization = Enum.TryParse<NormalizationMode>(normalization, out var n) ? n : NormalizationMode.Off,
            CrossfadeMs = crossfade_ms,
            DjRoleId = (ulong?)dj_role_id,
            Autoplay = autoplay,
            BoundTextChannelId = (ulong?)bound_text_channel_id,
            Locale = locale,
        };
    }
}
