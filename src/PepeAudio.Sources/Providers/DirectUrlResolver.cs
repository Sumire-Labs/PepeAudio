// SPDX-License-Identifier: Apache-2.0
using System.Runtime.CompilerServices;
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Enums;
using PepeAudio.Core.Exceptions;
using PepeAudio.Sources.Models;
using PepeAudio.Sources.Security;

namespace PepeAudio.Sources.Providers;

// Plays a direct https media URL after an SSRF check. FFmpeg reads it directly.
public sealed class DirectUrlResolver : ISourceResolver
{
    public SourceKind Kind => SourceKind.DirectUrl;
    public int Priority => 10;

    public bool CanHandle(ResolveRequest req)
        => req.Attachment is null && req.HasUrl
           && Uri.TryCreate(req.Url, UriKind.Absolute, out var u)
           && (u.Scheme == Uri.UriSchemeHttps || u.Scheme == Uri.UriSchemeHttp);

    public async IAsyncEnumerable<PlayableRef> ResolveAsync(
        ResolveRequest req, [EnumeratorCancellation] CancellationToken ct)
    {
        await Task.CompletedTask;
        if (!UrlSafetyGuard.IsSafeHttpUrl(req.Url, out var uri) || uri is null)
            throw new ResolveFailedException("URL が安全性チェックにより拒否されました（https かつ公開されている必要があります）。");

        var name = Path.GetFileName(uri.LocalPath);
        var title = string.IsNullOrWhiteSpace(name) ? uri.Host : name;
        yield return new PlayableRef(
            SourceKind.DirectUrl, uri.ToString(), Seekable: true,
            new TrackInfo(title, uri.Host, SourceKind.DirectUrl, uri.ToString(), 0, null, false, req.RequesterId));
    }
}
