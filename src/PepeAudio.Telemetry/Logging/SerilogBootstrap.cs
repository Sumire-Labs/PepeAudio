// SPDX-License-Identifier: Apache-2.0
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Serilog;
using Serilog.Events;
using Serilog.Formatting.Compact;
using Serilog.Sinks.SystemConsole.Themes;

namespace PepeAudio.Telemetry.Logging;

public static class SerilogBootstrap
{
    // Development: readable coloured console. Production: compact JSON to stdout.
    public static IServiceCollection AddPepeSerilog(
        this IServiceCollection services, IConfiguration config, IHostEnvironment env)
    {
        var level = Enum.TryParse<LogEventLevel>(config["Logging:MinimumLevel"], ignoreCase: true, out var l)
            ? l : LogEventLevel.Information;

        return services.AddSerilog(cfg =>
        {
            cfg.MinimumLevel.Is(level)
                .MinimumLevel.Override("Microsoft", LogEventLevel.Warning)
                .MinimumLevel.Override("System", LogEventLevel.Warning)
                .Enrich.FromLogContext();

            if (env.IsDevelopment())
                cfg.WriteTo.Async(a => a.Console(theme: AnsiConsoleTheme.Code));
            else
                cfg.WriteTo.Async(a => a.Console(new CompactJsonFormatter()));
        });
    }
}
