// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core.Contracts;
using PepeAudio.Sources.YtDlp;

namespace PepeAudio.Sources.Matching;

// Scores YouTube candidates against a target track and picks the best.
public static class YouTubeMatchScorer
{
    private static readonly string[] Variants =
        { "live", "cover", "remix", "sped up", "nightcore", "8d", "reverb", "karaoke", "instrumental", "acoustic" };

    private const int DurationToleranceMs = 15_000;

    public static YtDlpCandidate? Best(MatchQuery query, IReadOnlyList<YtDlpCandidate> candidates)
    {
        YtDlpCandidate? best = null;
        var bestScore = double.NegativeInfinity;
        foreach (var c in candidates)
        {
            var s = Score(query, c);
            if (s > bestScore) { bestScore = s; best = c; }
        }
        return best;
    }

    public static double Score(MatchQuery query, YtDlpCandidate c)
    {
        var qTitle = TextNormalizer.Tokens(query.Title);
        var titleSim = TextNormalizer.Jaccard(qTitle, TextNormalizer.Tokens(c.Title));

        var artistTokens = TextNormalizer.Tokens(query.Artist);
        var haystack = TextNormalizer.Normalize($"{c.Title} {c.Channel}");
        var artistMatch = artistTokens.Count > 0 && artistTokens.All(haystack.Contains) ? 1.0 : 0.0;

        var durProx = query.DurationMs > 0 && c.DurationMs > 0
            ? 1 - Math.Min(1.0, Math.Abs(query.DurationMs - c.DurationMs) / (double)DurationToleranceMs)
            : 0.5;

        var channel = (c.Channel ?? "").ToLowerInvariant();
        var channelSignal = channel.EndsWith(" - topic") ? 1.0 : channel.Contains("vevo") ? 0.5 : 0.0;

        var penalty = HasForeignVariant(query.Title, c.Title) ? 0.3 : 0.0;

        return 0.40 * titleSim + 0.25 * artistMatch + 0.25 * durProx + 0.10 * channelSignal - penalty;
    }

    private static bool HasForeignVariant(string queryTitle, string candidateTitle)
    {
        var q = queryTitle.ToLowerInvariant();
        var c = candidateTitle.ToLowerInvariant();
        return Variants.Any(v => c.Contains(v) && !q.Contains(v));
    }
}
