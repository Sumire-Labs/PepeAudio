// SPDX-License-Identifier: Apache-2.0
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Diagnostics.HealthChecks;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Options;
using PepeAudio.Application;
using PepeAudio.Audio;
using PepeAudio.Cache;
using PepeAudio.Core.Observability;
using PepeAudio.Core.Sharding;
using PepeAudio.Data;
using PepeAudio.Discord;
using PepeAudio.Host.Coordination;
using PepeAudio.Host.Health;
using PepeAudio.Host.Lifecycle;
using PepeAudio.Host.Sharding;
using PepeAudio.Sources;
using PepeAudio.Telemetry;
using PepeAudio.Telemetry.Logging;
using PepeAudio.Web;

var builder = WebApplication.CreateBuilder(args);

builder.Configuration
    .AddJsonFile("config/appsettings.json", optional: true, reloadOnChange: false)
    .AddJsonFile($"config/appsettings.{builder.Environment.EnvironmentName}.json", optional: true, reloadOnChange: false)
    .AddEnvironmentVariables()
    .AddKeyPerFile("/run/secrets", optional: true);

builder.Services.AddPepeSerilog(builder.Configuration, builder.Environment);

// A transient failure in a background service (heartbeat/consumer) must not kill the bot.
builder.Services.Configure<HostOptions>(o =>
    o.BackgroundServiceExceptionBehavior = BackgroundServiceExceptionBehavior.Ignore);

builder.Services.AddPepeDiscord(builder.Configuration);
builder.Services.AddSingleton(sp =>
    ShardedClientFactory.Create(sp.GetRequiredService<IOptions<DiscordOptions>>().Value));

builder.Services.AddSingleton<InstanceIdentity>();
builder.Services.AddSingleton<BotHealthState>();
builder.Services.AddSingleton<PlaybackRestorer>();
builder.Services.AddHealthChecks().AddCheck<GatewayHealthCheck>("gateway", tags: new[] { "ready" });
builder.Services.AddPepeTelemetry(builder.Configuration, builder.Environment);
builder.Services.AddSingleton<IShardTopology>(sp =>
{
    var opt = sp.GetRequiredService<IOptions<DiscordOptions>>().Value;
    return new ShardTopology(opt.TotalShards, opt.ShardIds);
});

builder.Services.AddPepeCache(builder.Configuration);
builder.Services.AddPepeData(builder.Configuration);
builder.Services.AddPepeSources(builder.Configuration);
builder.Services.AddPepeAudio(builder.Configuration);
builder.Services.AddPepeApplication();
builder.Services.AddPepeWeb(builder.Configuration);

builder.Services.AddHostedService<BotHostedService>();
builder.Services.AddHostedService<ShardHeartbeatService>();
builder.Services.AddHostedService<ControlBusConsumerService>();
builder.Services.AddHostedService<PlayerCardUpdater>();
builder.Services.AddHostedService<VoicePresenceService>();
builder.Services.AddHostedService<PresenceService>();
// Registered last => its StopAsync runs first (drain voice before logout).
builder.Services.AddHostedService<GracefulDrainService>();

var app = builder.Build();

PepeMetrics.RegisterActiveVoices(() => app.Services.GetRequiredService<IPlayerManager>().ActiveCount);

// Ops endpoints (always on): liveness = process up, readiness = shards ready, metrics = Prometheus.
app.MapHealthChecks("/healthz", new HealthCheckOptions { Predicate = _ => false });
app.MapHealthChecks("/readyz", new HealthCheckOptions { Predicate = c => c.Tags.Contains("ready") });
app.MapPrometheusScrapingEndpoint();

app.MapPepeWeb();
await app.RunAsync();
