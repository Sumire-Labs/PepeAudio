// SPDX-License-Identifier: Apache-2.0
using System.Reflection;
using DbUp;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;

namespace PepeAudio.Data;

// Runs forward-only DbUp migrations at startup. Best-effort in dev: a missing DB
// logs a warning rather than crashing. Production wires a Valkey single-run lock.
public sealed class MigrationRunner
{
    private readonly PostgresOptions _opt;
    private readonly ILogger<MigrationRunner> _log;

    public MigrationRunner(IOptions<PostgresOptions> opt, ILogger<MigrationRunner> log)
    {
        _opt = opt.Value;
        _log = log;
    }

    public void Run()
    {
        if (string.IsNullOrWhiteSpace(_opt.Postgres))
        {
            _log.LogWarning("Skipping migrations: no Postgres connection string.");
            return;
        }
        try
        {
            var upgrader = DeployChanges.To
                .PostgresqlDatabase(_opt.Postgres)
                .WithScriptsEmbeddedInAssembly(Assembly.GetExecutingAssembly())
                .WithTransactionPerScript()
                .LogToNowhere()
                .Build();

            var result = upgrader.PerformUpgrade();
            if (!result.Successful)
                _log.LogWarning("Migrations did not run: {Error}", result.Error?.Message);
            else
                _log.LogInformation("Database schema up to date.");
        }
        catch (Exception ex)
        {
            _log.LogWarning(ex, "Migrations skipped: Postgres unavailable.");
        }
    }
}
