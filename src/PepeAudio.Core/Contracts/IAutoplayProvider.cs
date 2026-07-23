// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Core.Contracts;

// Supplies related tracks to keep playback going after the queue drains (radio / mix).
public interface IAutoplayProvider
{
    Task<IReadOnlyList<PlayableRef>> RelatedAsync(PlayableRef seed, int max, CancellationToken ct);
}
