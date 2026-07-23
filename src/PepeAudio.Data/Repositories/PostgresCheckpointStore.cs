// SPDX-License-Identifier: Apache-2.0
using System.Text.Json;
using Dapper;
using Microsoft.Extensions.Logging;
using PepeAudio.Core.Contracts;

namespace PepeAudio.Data.Repositories;

// Durable playback checkpoints in PostgreSQL. Best-effort: an unavailable DB is
// a no-op (checkpoints are an optimization for resume-after-restart).
public sealed class PostgresCheckpointStore : ICheckpointStore
{
    private readonly INpgsqlDataSourceProvider _db;
    private readonly ILogger<PostgresCheckpointStore> _log;

    public PostgresCheckpointStore(INpgsqlDataSourceProvider db, ILogger<PostgresCheckpointStore> log)
    {
        _db = db;
        _log = log;
    }

    public async Task SaveAsync(PlayerCheckpoint checkpoint, CancellationToken ct)
    {
        var src = _db.DataSource;
        if (src is null) return;
        try
        {
            await using var conn = await src.OpenConnectionAsync(ct);
            await conn.ExecuteAsync(new CommandDefinition(
                "INSERT INTO guild_player_state(guild_id, voice_channel_id, snapshot, updated_at) " +
                "VALUES (@gid, @vc, @snap::jsonb, now()) " +
                "ON CONFLICT (guild_id) DO UPDATE SET voice_channel_id=@vc, snapshot=@snap::jsonb, updated_at=now()",
                new { gid = (long)checkpoint.GuildId, vc = (long)checkpoint.VoiceChannelId, snap = JsonSerializer.Serialize(checkpoint) }, cancellationToken: ct));
        }
        catch (Exception ex) { _log.LogDebug(ex, "Checkpoint save skipped for {Guild}", checkpoint.GuildId); }
    }

    public async Task<IReadOnlyList<PlayerCheckpoint>> LoadAllAsync(CancellationToken ct)
    {
        var src = _db.DataSource;
        if (src is null) return Array.Empty<PlayerCheckpoint>();
        try
        {
            await using var conn = await src.OpenConnectionAsync(ct);
            var rows = await conn.QueryAsync<string>(new CommandDefinition(
                "SELECT snapshot FROM guild_player_state", cancellationToken: ct));
            var list = new List<PlayerCheckpoint>();
            foreach (var s in rows)
            {
                // One corrupt row must not abort resume for every other guild.
                try { if (JsonSerializer.Deserialize<PlayerCheckpoint>(s) is { } cp) list.Add(cp); }
                catch (Exception ex) { _log.LogWarning(ex, "Skipping a corrupt checkpoint row"); }
            }
            return list;
        }
        catch (Exception ex) { _log.LogDebug(ex, "Checkpoint load skipped"); return Array.Empty<PlayerCheckpoint>(); }
    }

    public async Task DeleteAsync(ulong guildId, CancellationToken ct)
    {
        var src = _db.DataSource;
        if (src is null) return;
        try
        {
            await using var conn = await src.OpenConnectionAsync(ct);
            await conn.ExecuteAsync(new CommandDefinition("DELETE FROM guild_player_state WHERE guild_id = @gid", new { gid = (long)guildId }, cancellationToken: ct));
        }
        catch (Exception ex) { _log.LogDebug(ex, "Checkpoint delete skipped for {Guild}", guildId); }
    }
}
