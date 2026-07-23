// SPDX-License-Identifier: Apache-2.0
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using PepeAudio.Core.Contracts;
using PepeAudio.Sources.Cache;
using PepeAudio.Sources.Matching;
using PepeAudio.Sources.Metadata;
using PepeAudio.Sources.Providers;
using PepeAudio.Sources.YtDlp;

namespace PepeAudio.Sources;

public static class SourcesServiceCollectionExtensions
{
    public static IServiceCollection AddPepeSources(this IServiceCollection services, IConfiguration config)
    {
        services.AddHttpClient();
        services.Configure<YtDlpOptions>(config.GetSection(YtDlpOptions.Section));
        services.Configure<SpotifyOptions>(config.GetSection(SpotifyOptions.Section));
        services.Configure<AppleMusicOptions>(config.GetSection(AppleMusicOptions.Section));

        services.AddSingleton<IYtDlpClient, YtDlpClient>();
        services.AddSingleton<IStreamProvider, YtDlpStreamProvider>();
        services.AddSingleton<IAutoplayProvider, YouTubeAutoplayProvider>();
        services.AddSingleton<IYouTubeMatcher, YouTubeMatcher>();
        services.AddSingleton<SpotifyMetadataClient>();
        services.AddSingleton<SpotifyPublicClient>();
        services.AddSingleton<AppleMusicMetadataClient>();
        services.AddSingleton<AppleMusicPublicClient>();

        services.AddSingleton<ValkeyTrackCache>();
        services.AddSingleton<PostgresTrackCacheStore>();
        services.AddSingleton<ITrackCache, TwoTierTrackCache>();

        services.AddSingleton<ISourceResolver, AttachmentResolver>();
        services.AddSingleton<ISourceResolver, SpotifyResolver>();
        services.AddSingleton<ISourceResolver, AppleMusicResolver>();
        services.AddSingleton<ISourceResolver, YouTubeResolver>();
        services.AddSingleton<ISourceResolver, SoundCloudResolver>();
        services.AddSingleton<ISourceResolver, DirectUrlResolver>();
        services.AddSingleton<IResolverRegistry, SourceResolverRegistry>();
        return services;
    }
}
