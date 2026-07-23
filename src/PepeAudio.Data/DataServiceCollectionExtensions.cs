// SPDX-License-Identifier: Apache-2.0
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using PepeAudio.Core.Contracts;
using PepeAudio.Data.Repositories;

namespace PepeAudio.Data;

public static class DataServiceCollectionExtensions
{
    public static IServiceCollection AddPepeData(this IServiceCollection services, IConfiguration config)
    {
        services.Configure<PostgresOptions>(config.GetSection(PostgresOptions.Section));
        services.AddSingleton<INpgsqlDataSourceProvider, NpgsqlDataSourceProvider>();
        services.AddSingleton<MigrationRunner>();
        services.AddSingleton<IGuildSettingsRepository, GuildSettingsRepository>();
        services.AddSingleton<ICheckpointStore, PostgresCheckpointStore>();
        services.AddSingleton<IPlaylistRepository, PlaylistRepository>();
        return services;
    }
}
