// SPDX-License-Identifier: Apache-2.0
using System.Runtime.CompilerServices;
using Microsoft.Extensions.Logging;
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Exceptions;
using PepeAudio.Sources.Models;

namespace PepeAudio.Sources;

public sealed class SourceResolverRegistry : IResolverRegistry
{
    private readonly IReadOnlyList<ISourceResolver> _resolvers;
    private readonly ILogger<SourceResolverRegistry> _log;

    public SourceResolverRegistry(IEnumerable<ISourceResolver> resolvers, ILogger<SourceResolverRegistry> log)
    {
        _resolvers = resolvers.OrderByDescending(r => r.Priority).ToArray();
        _log = log;
    }

    public async IAsyncEnumerable<PlayableRef> ResolveAsync(
        ResolveRequest req, [EnumeratorCancellation] CancellationToken ct)
    {
        var resolver = _resolvers.FirstOrDefault(r => r.CanHandle(req))
            ?? throw new ResolveFailedException("この入力に対応できるソースがありません。");

        _log.LogDebug("Resolving with {Resolver}", resolver.Kind);
        await foreach (var track in resolver.ResolveAsync(req, ct).WithCancellation(ct))
            yield return track;
    }
}
