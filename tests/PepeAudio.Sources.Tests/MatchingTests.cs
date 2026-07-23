// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core.Contracts;
using PepeAudio.Sources.Matching;
using PepeAudio.Sources.YtDlp;
using Xunit;

namespace PepeAudio.Sources.Tests;

public class MatchingTests
{
    [Fact]
    public void Normalize_strips_noise_brackets_and_diacritics()
    {
        var n = TextNormalizer.Normalize("Beyoncé - Halo (Official Video) [HD]");
        Assert.Contains("beyonce", n);
        Assert.Contains("halo", n);
        Assert.DoesNotContain("official", n);
        Assert.DoesNotContain("hd", n);
        Assert.DoesNotContain("(", n);
    }

    [Fact]
    public void Jaccard_bounds()
    {
        var a = TextNormalizer.Tokens("shape of you");
        Assert.Equal(1.0, TextNormalizer.Jaccard(a, TextNormalizer.Tokens("Shape Of You")));
        Assert.Equal(0.0, TextNormalizer.Jaccard(a, TextNormalizer.Tokens("totally different words")));
    }

    [Fact]
    public void Scorer_prefers_official_over_variants()
    {
        var query = new MatchQuery("Shape of You", "Ed Sheeran", 233_000, null);
        var candidates = new List<YtDlpCandidate>
        {
            new("v1", "Shape of You (Nightcore)", "Random", 200_000, "https://youtu.be/v1"),
            new("v2", "Ed Sheeran - Shape of You (Official Video)", "Ed Sheeran", 240_000, "https://youtu.be/v2"),
            new("v3", "Shape of You (Live at Wembley)", "Fan", 320_000, "https://youtu.be/v3"),
        };

        var best = YouTubeMatchScorer.Best(query, candidates);
        Assert.NotNull(best);
        Assert.Equal("v2", best!.Id);
    }

    [Fact]
    public void Scorer_returns_null_for_no_candidates()
        => Assert.Null(YouTubeMatchScorer.Best(new MatchQuery("x", "y", 0, null), Array.Empty<YtDlpCandidate>()));
}
