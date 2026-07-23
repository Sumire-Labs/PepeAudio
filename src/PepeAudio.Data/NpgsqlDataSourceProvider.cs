// SPDX-License-Identifier: Apache-2.0
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using Npgsql;

namespace PepeAudio.Data;

public interface INpgsqlDataSourceProvider
{
    // Null when Postgres is not configured/available (best-effort in dev).
    NpgsqlDataSource? DataSource { get; }
}

public sealed class NpgsqlDataSourceProvider : INpgsqlDataSourceProvider, IDisposable
{
    private readonly Lazy<NpgsqlDataSource?> _source;

    public NpgsqlDataSourceProvider(IOptions<PostgresOptions> opt, ILogger<NpgsqlDataSourceProvider> log)
    {
        _source = new Lazy<NpgsqlDataSource?>(() =>
        {
            var conn = opt.Value.Postgres;
            if (string.IsNullOrWhiteSpace(conn))
            {
                log.LogWarning("Postgres connection string not set; running without persistence.");
                return null;
            }
            return new NpgsqlDataSourceBuilder(conn).Build();
        });
    }

    public NpgsqlDataSource? DataSource => _source.Value;

    public void Dispose() => (_source.IsValueCreated ? _source.Value : null)?.Dispose();
}
