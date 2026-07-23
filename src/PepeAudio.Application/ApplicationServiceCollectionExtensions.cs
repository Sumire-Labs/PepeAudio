// SPDX-License-Identifier: Apache-2.0
using Microsoft.Extensions.DependencyInjection;
using PepeAudio.Application.Playback;

namespace PepeAudio.Application;

public static class ApplicationServiceCollectionExtensions
{
    public static IServiceCollection AddPepeApplication(this IServiceCollection services)
    {
        services.AddSingleton<ShutdownState>();
        services.AddSingleton<IPlaybackService, PlaybackService>();
        return services;
    }
}
