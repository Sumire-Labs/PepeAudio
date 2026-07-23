// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core.Enums;

namespace PepeAudio.Core.Contracts;

public sealed record TrackInfo(
    string Title,
    string Artist,
    SourceKind Source,
    string Url,
    long DurationMs,
    string? ThumbnailUrl,
    bool IsLive,
    ulong RequestedBy);
