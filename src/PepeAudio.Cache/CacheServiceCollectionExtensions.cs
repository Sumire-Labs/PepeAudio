// SPDX-License-Identifier: Apache-2.0
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;

namespace PepeAudio.Cache;

public static class CacheServiceCollectionExtensions
{
    public static IServiceCollection AddPepeCache(this IServiceCollection services, IConfiguration config)
    {
        services.Configure<ValkeyOptions>(config.GetSection(ValkeyOptions.Section));
        services.AddSingleton<IValkeyConnection, ValkeyConnection>();
        services.AddSingleton<IPlayerStateStore, PlayerStateStore>();
        services.AddSingleton<IValkeyLock, ValkeyLock>();
        services.AddSingleton<IShardRegistry, ValkeyShardRegistry>();
        services.AddSingleton<ICommandBus, ValkeyCommandBus>();
        return services;
    }
}
