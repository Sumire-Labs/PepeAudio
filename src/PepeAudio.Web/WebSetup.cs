// SPDX-License-Identifier: Apache-2.0
using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using PepeAudio.Web.Api;
using PepeAudio.Web.Auth;
using PepeAudio.Web.Hubs;
using PepeAudio.Web.Realtime;
using StackExchange.Redis;

namespace PepeAudio.Web;

public static class WebSetup
{
    public static IServiceCollection AddPepeWeb(this IServiceCollection services, IConfiguration config)
    {
        services.Configure<WebOptions>(config.GetSection(WebOptions.Section));
        var opt = config.GetSection(WebOptions.Section).Get<WebOptions>() ?? new WebOptions();
        if (!opt.Enabled) return services;

        services.AddMemoryCache();
        services.AddHttpClient();
        services.AddSingleton<JwtIssuer>();
        services.AddSingleton<DiscordApiClient>();
        services.AddHostedService<PlayerBroadcastService>();

        services.ConfigureHttpJsonOptions(o =>
            o.SerializerOptions.Converters.Add(new Serialization.UInt64StringConverter()));

        services.AddAuthentication(JwtBearerDefaults.AuthenticationScheme)
            .AddJwtBearer(o =>
            {
                o.MapInboundClaims = false;
                o.TokenValidationParameters = JwtIssuer.ValidationParameters(opt);
                o.Events = new JwtBearerEvents
                {
                    OnMessageReceived = ctx =>
                    {
                        var token = ctx.Request.Cookies[opt.SessionCookieName];
                        if (string.IsNullOrEmpty(token) && ctx.Request.Path.StartsWithSegments("/hubs")
                            && ctx.Request.Query.TryGetValue("access_token", out var q))
                            token = q;
                        if (!string.IsNullOrEmpty(token)) ctx.Token = token;
                        return Task.CompletedTask;
                    },
                };
            });
        services.AddAuthorization();

        var signalr = services.AddSignalR()
            .AddJsonProtocol(o =>
            {
                o.PayloadSerializerOptions.PropertyNamingPolicy = System.Text.Json.JsonNamingPolicy.CamelCase;
                o.PayloadSerializerOptions.Converters.Add(new Serialization.UInt64StringConverter());
            });
        var valkey = config["ConnectionStrings:Valkey"];
        if (!string.IsNullOrWhiteSpace(valkey))
            signalr.AddStackExchangeRedis(valkey, o =>
            {
                o.Configuration.AbortOnConnectFail = false;
                o.Configuration.ChannelPrefix = RedisChannel.Literal("PepeAudio");
            });
        return services;
    }

    public static WebApplication MapPepeWeb(this WebApplication app)
    {
        var opt = app.Services.GetRequiredService<Microsoft.Extensions.Options.IOptions<WebOptions>>().Value;
        if (!opt.Enabled) return app;

        app.UseAuthentication();
        app.UseAuthorization();
        app.MapAuthEndpoints();
        app.MapGuildsEndpoints();
        app.MapAdminEndpoints();
        app.MapHub<PlayerHub>("/hubs/player");
        return app;
    }
}
