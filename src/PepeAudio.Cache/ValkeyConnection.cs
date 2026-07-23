// SPDX-License-Identifier: Apache-2.0
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using StackExchange.Redis;

namespace PepeAudio.Cache;

public interface IValkeyConnection
{
    // Returns a connected database, or null if Valkey is unavailable (best-effort).
    IDatabase? TryGetDatabase();
}

public sealed class ValkeyConnection : IValkeyConnection, IDisposable
{
    private const int TimeoutMs = 2000;

    private readonly Lazy<IConnectionMultiplexer?> _mux;

    public ValkeyConnection(IOptions<ValkeyOptions> opt, ILogger<ValkeyConnection> log)
    {
        _mux = new Lazy<IConnectionMultiplexer?>(() =>
        {
            var conn = opt.Value.Valkey;
            if (string.IsNullOrWhiteSpace(conn))
            {
                log.LogWarning("Valkey connection string not set; running without cache/coordination.");
                return null;
            }
            try
            {
                var cfg = ConfigurationOptions.Parse(conn);
                cfg.AbortOnConnectFail = false;
                cfg.ConnectTimeout = TimeoutMs;
                cfg.ConnectRetry = 1;
                cfg.SyncTimeout = TimeoutMs;
                return ConnectionMultiplexer.Connect(cfg);
            }
            catch (Exception ex)
            {
                log.LogWarning(ex, "Valkey connect failed; continuing without cache/coordination.");
                return null;
            }
        });
    }

    // Null only when Valkey is unconfigured or the initial connect failed. A transient
    // disconnect still returns a database — StackExchange.Redis handles reconnection and
    // callers already treat command failures as best-effort.
    public IDatabase? TryGetDatabase() => _mux.Value?.GetDatabase();

    public void Dispose() => (_mux.IsValueCreated ? _mux.Value : null)?.Dispose();
}
