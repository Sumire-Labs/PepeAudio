// SPDX-License-Identifier: Apache-2.0
using Discord;
using Discord.WebSocket;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using PepeAudio.Cache;
using PepeAudio.Data;
using PepeAudio.Discord;
using PepeAudio.Discord.Interactions;
using PepeAudio.Host.Coordination;

namespace PepeAudio.Host.Lifecycle;

public sealed class BotHostedService : IHostedService
{
    private readonly DiscordShardedClient _client;
    private readonly DiscordOptions _opt;
    private readonly InteractionHandler _handler;
    private readonly CommandRegistrar _registrar;
    private readonly MigrationRunner _migrations;
    private readonly IValkeyLock _lock;
    private readonly BotHealthState _health;
    private readonly PlaybackRestorer _restorer;
    private readonly ILogger<BotHostedService> _log;
    private int _commandsRegistered;

    public BotHostedService(DiscordShardedClient client, IOptions<DiscordOptions> opt, InteractionHandler handler,
        CommandRegistrar registrar, MigrationRunner migrations, IValkeyLock valkeyLock,
        BotHealthState health, PlaybackRestorer restorer, ILogger<BotHostedService> log)
    {
        _client = client;
        _opt = opt.Value;
        _handler = handler;
        _registrar = registrar;
        _migrations = migrations;
        _lock = valkeyLock;
        _health = health;
        _restorer = restorer;
        _log = log;
    }

    public async Task StartAsync(CancellationToken cancellationToken)
    {
        await _lock.RunOnceAsync(ValkeyKeys.LockMigrate, TimeSpan.FromMinutes(2),
            () => { _migrations.Run(); return Task.CompletedTask; });

        if (string.IsNullOrWhiteSpace(_opt.Token))
        {
            _log.LogError("Discord:Token is not set. The bot will not log in. Set DISCORD__TOKEN and restart.");
            return;
        }

        _client.Log += OnLog;
        _client.ShardReady += OnShardReady;
        await _handler.InitializeAsync();
        await _client.LoginAsync(TokenType.Bot, _opt.Token);
        await _client.StartAsync();
        _health.ExpectedShards = _client.Shards.Count;
        _health.LoggedIn = true;
    }

    public async Task StopAsync(CancellationToken cancellationToken)
    {
        _health.Reset();
        try { await _client.LogoutAsync(); await _client.StopAsync(); }
        catch (Exception ex) { _log.LogWarning(ex, "Error during shutdown"); }
    }

    private async Task OnShardReady(DiscordSocketClient shard)
    {
        _health.MarkShardReady(shard.ShardId);
        if (Interlocked.Exchange(ref _commandsRegistered, 1) == 0)
            await _lock.RunOnceAsync(ValkeyKeys.LockCommandReg, TimeSpan.FromMinutes(2), _registrar.RegisterAsync);
        _log.LogInformation("Shard {Shard} ready ({Guilds} guilds)", shard.ShardId, shard.Guilds.Count);
        await _restorer.RestoreShardAsync(shard.ShardId);
    }

    private Task OnLog(LogMessage msg)
    {
        var level = msg.Severity switch
        {
            LogSeverity.Critical => LogLevel.Critical,
            LogSeverity.Error => LogLevel.Error,
            LogSeverity.Warning => LogLevel.Warning,
            LogSeverity.Info => LogLevel.Information,
            LogSeverity.Verbose => LogLevel.Debug,
            _ => LogLevel.Trace,
        };
        _log.Log(level, msg.Exception, "[{Source}] {Message}", msg.Source, msg.Message);
        return Task.CompletedTask;
    }
}
