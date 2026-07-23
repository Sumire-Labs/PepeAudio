// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core.Enums;

namespace PepeAudio.Core.Contracts;

// A player control routed through the unified control chain. Epoch guards against
// commands delivered after an ownership handover.
public sealed record ControlEnvelope(
    ulong GuildId,
    PlayerControl Control,
    string? Arg,
    ulong ActorUserId,
    long Epoch,
    DateTimeOffset IssuedAt);
