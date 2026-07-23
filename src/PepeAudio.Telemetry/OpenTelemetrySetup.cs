// SPDX-License-Identifier: Apache-2.0
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using OpenTelemetry.Metrics;
using OpenTelemetry.Trace;

namespace PepeAudio.Telemetry;

public static class OpenTelemetrySetup
{
    // Metrics -> Prometheus (/metrics). Traces -> OTLP if configured, else the
    // console in Development. Ops endpoints are excluded from traces.
    public static IServiceCollection AddPepeTelemetry(
        this IServiceCollection services, IConfiguration config, IHostEnvironment env)
    {
        var otlp = config["Telemetry:OtlpEndpoint"]
            ?? Environment.GetEnvironmentVariable("OTEL_EXPORTER_OTLP_ENDPOINT");

        services.AddOpenTelemetry()
            .WithMetrics(m => m
                .AddMeter("PepeAudio")
                .AddAspNetCoreInstrumentation()
                .AddRuntimeInstrumentation()
                .AddPrometheusExporter())
            .WithTracing(t =>
            {
                t.AddSource("PepeAudio")
                    .AddAspNetCoreInstrumentation(o => o.Filter = IsTraceable)
                    .AddHttpClientInstrumentation();
                if (!string.IsNullOrWhiteSpace(otlp))
                    t.AddOtlpExporter();
                else if (env.IsDevelopment())
                    t.AddConsoleExporter();
            });
        return services;
    }

    private static bool IsTraceable(HttpContext ctx) =>
        !ctx.Request.Path.StartsWithSegments("/metrics")
        && !ctx.Request.Path.StartsWithSegments("/healthz")
        && !ctx.Request.Path.StartsWithSegments("/readyz");
}
