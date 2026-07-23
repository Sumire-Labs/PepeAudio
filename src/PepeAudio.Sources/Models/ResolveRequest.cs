// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Sources.Models;

public sealed record AttachmentRef(string Url, string FileName, string? ContentType, long Size);

public sealed record ResolveRequest(string? Url, AttachmentRef? Attachment, ulong RequesterId)
{
    public bool HasUrl => !string.IsNullOrWhiteSpace(Url);
}
