// SPDX-License-Identifier: Apache-2.0
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using PepeAudio.Audio.Effects;

namespace PepeAudio.Audio;

public static class AudioServiceCollectionExtensions
{
    public static IServiceCollection AddPepeAudio(this IServiceCollection services, IConfiguration config)
    {
        services.Configure<AudioOptions>(config.GetSection(AudioOptions.Section));
        services.AddSingleton<IHeSuViPresetLibrary, HeSuViPresetLibrary>();
        services.AddSingleton<EffectChainBuilder>();
        services.AddSingleton<IPlayerManager, PlayerManager>();
        return services;
    }
}
