// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Core.Contracts;

// A saved playlist track. SourceUrl may be a provider URL or a bare "artist title" search
// string (collection URLs are rewritten to the latter on save). Source is the SourceKind ordinal.
public sealed record PlaylistTrack(
    string SourceUrl,
    string Title,
    string Artist,
    string? ThumbnailUrl,
    int Source,
    long? DurationMs);

public sealed record PlaylistSummary(string Id, string Name, int TrackCount, long UpdatedAt);

public sealed record PlaylistDetail(string Id, string Name, int TrackCount, long UpdatedAt, IReadOnlyList<PlaylistTrack> Tracks);
