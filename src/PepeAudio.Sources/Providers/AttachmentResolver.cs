// SPDX-License-Identifier: Apache-2.0
using System.Runtime.CompilerServices;
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Enums;
using PepeAudio.Core.Exceptions;
using PepeAudio.Sources.Models;

namespace PepeAudio.Sources.Providers;

// Plays an uploaded Discord attachment. FFmpeg reads the CDN URL directly.
public sealed class AttachmentResolver : ISourceResolver
{
    private static readonly string[] Allowed = { ".mp3", ".flac", ".wav", ".ogg", ".m4a", ".opus" };

    public SourceKind Kind => SourceKind.Attachment;
    public int Priority => 30;

    public bool CanHandle(ResolveRequest req) => req.Attachment is not null;

    public async IAsyncEnumerable<PlayableRef> ResolveAsync(
        ResolveRequest req, [EnumeratorCancellation] CancellationToken ct)
    {
        await Task.CompletedTask;
        var att = req.Attachment!;
        var ext = Path.GetExtension(att.FileName).ToLowerInvariant();
        var looksAudio = Allowed.Contains(ext)
            || (att.ContentType?.StartsWith("audio/", StringComparison.OrdinalIgnoreCase) ?? false);
        if (!looksAudio)
            throw new ResolveFailedException("添付ファイルは対応していない音声形式です。");

        yield return new PlayableRef(
            SourceKind.Attachment, att.Url, Seekable: true,
            new TrackInfo(att.FileName, "Attachment", SourceKind.Attachment, att.Url, 0, null, false, req.RequesterId));
    }
}
