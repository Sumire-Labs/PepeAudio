// SPDX-License-Identifier: Apache-2.0
using Discord.Interactions;
using Discord.WebSocket;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using PepeAudio.Discord.Components;
using PepeAudio.Discord.Interactions;

namespace PepeAudio.Discord;

public static class DiscordServiceCollectionExtensions
{
    // Registers Discord interaction plumbing. The DiscordShardedClient itself is
    // provided by the host (sharding composition).
    public static IServiceCollection AddPepeDiscord(this IServiceCollection services, IConfiguration config)
    {
        services.Configure<DiscordOptions>(config.GetSection(DiscordOptions.Section));

        services.AddSingleton(sp => new InteractionService(
            sp.GetRequiredService<DiscordShardedClient>(),
            new InteractionServiceConfig { DefaultRunMode = RunMode.Async, UseCompiledLambda = true }));

        services.AddSingleton<InteractionHandler>();
        services.AddSingleton<CommandRegistrar>();
        services.AddSingleton<NowPlayingService>();
        return services;
    }
}
