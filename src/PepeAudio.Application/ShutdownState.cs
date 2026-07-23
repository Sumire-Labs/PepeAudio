// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Application;

// Flips true when the host begins draining, so new /play requests are refused
// while in-flight voice sessions are torn down cleanly.
public sealed class ShutdownState
{
    private volatile bool _draining;
    public bool IsDraining => _draining;
    public void BeginDraining() => _draining = true;
}
