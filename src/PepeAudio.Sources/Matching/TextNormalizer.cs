// SPDX-License-Identifier: Apache-2.0
using System.Globalization;
using System.Text;
using System.Text.RegularExpressions;

namespace PepeAudio.Sources.Matching;

// Normalizes titles/artists for matching: lowercase, strip diacritics, drop
// bracketed noise (official video/audio/lyrics/...), unify feat., collapse space.
public static partial class TextNormalizer
{
    private static readonly string[] Noise =
    {
        "official music video", "official video", "official audio", "lyric video", "lyrics",
        "audio", "video", "visualizer", "remastered", "remaster", "hd", "4k", "mv",
    };

    public static string Normalize(string? input)
    {
        var s = (input ?? "").ToLowerInvariant();
        s = StripDiacritics(s);
        s = Bracketed().Replace(s, " ");
        s = s.Replace("feat.", " ").Replace("ft.", " ").Replace("featuring", " ");
        foreach (var n in Noise) s = s.Replace(n, " ");
        s = NonAlnum().Replace(s, " ");
        return Spaces().Replace(s, " ").Trim();
    }

    public static HashSet<string> Tokens(string? input)
        => Normalize(input).Split(' ', StringSplitOptions.RemoveEmptyEntries).ToHashSet();

    public static double Jaccard(IReadOnlySet<string> a, IReadOnlySet<string> b)
    {
        if (a.Count == 0 || b.Count == 0) return 0;
        var inter = a.Count(b.Contains);
        return (double)inter / (a.Count + b.Count - inter);
    }

    private static string StripDiacritics(string s)
    {
        var d = s.Normalize(NormalizationForm.FormD);
        var sb = new StringBuilder(d.Length);
        foreach (var c in d)
            if (CharUnicodeInfo.GetUnicodeCategory(c) != UnicodeCategory.NonSpacingMark)
                sb.Append(c);
        return sb.ToString().Normalize(NormalizationForm.FormC);
    }

    [GeneratedRegex(@"[\(\[\{][^\)\]\}]*[\)\]\}]")]
    private static partial Regex Bracketed();

    // Keep letters/digits of every script (CJK, Cyrillic, Greek, ...) and drop only
    // punctuation/symbols — an ASCII-only class wipes non-Latin titles to nothing.
    [GeneratedRegex(@"[^\p{L}\p{N}\s]")]
    private static partial Regex NonAlnum();

    [GeneratedRegex(@"\s+")]
    private static partial Regex Spaces();
}
