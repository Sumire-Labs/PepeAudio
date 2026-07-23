// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Enums;
using PepeAudio.Sources.Models;

namespace PepeAudio.Sources;

public interface ISourceResolver
{
    SourceKind Kind { get; }
    int Priority { get; }
    bool CanHandle(ResolveRequest req);
    IAsyncEnumerable<PlayableRef> ResolveAsync(ResolveRequest req, CancellationToken ct);
}

public interface IResolverRegistry
{
    IAsyncEnumerable<PlayableRef> ResolveAsync(ResolveRequest req, CancellationToken ct);
}
