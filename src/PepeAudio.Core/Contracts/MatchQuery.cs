// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Core.Contracts;

// Emitted by DRM metadata sources (Spotify/Apple Music) for the YouTube matcher.
public sealed record MatchQuery(
    string Title,
    string Artist,
    long DurationMs,
    string? Isrc);
