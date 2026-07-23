// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core.Contracts;

namespace PepeAudio.Application.Playback;

public sealed record UploadedFile(string Url, string FileName, string? ContentType, long Size);

public sealed record PlayRequest(
    ulong GuildId,
    ulong VoiceChannelId,
    ulong TextChannelId,
    ulong RequesterId,
    string? Url,
    UploadedFile? File);

public sealed record PlayResult(TrackInfo First, int EnqueuedCount);
