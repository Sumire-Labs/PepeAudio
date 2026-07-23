// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Core.Contracts;

// Resolves a PlayableRef to a concrete stream URL / path just before playback.
public interface IStreamProvider
{
    Task<string> ResolveStreamAsync(PlayableRef track, CancellationToken ct);
}
