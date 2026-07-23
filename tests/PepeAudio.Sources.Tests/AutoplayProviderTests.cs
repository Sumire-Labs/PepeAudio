// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Enums;
using PepeAudio.Sources.Providers;
using PepeAudio.Sources.YtDlp;
using Xunit;

namespace PepeAudio.Sources.Tests;

public class AutoplayProviderTests
{
    private sealed class FakeYtDlp : IYtDlpClient
    {
        public string? MixUrl;
        public IReadOnlyList<YtDlpCandidate> Entries = Array.Empty<YtDlpCandidate>();
        public Task<YtDlpResolved?> GetTrackAsync(string input, CancellationToken ct) => Task.FromResult<YtDlpResolved?>(null);
        public Task<string?> GetStreamUrlAsync(string input, CancellationToken ct) => Task.FromResult<string?>(null);
        public Task<IReadOnlyList<YtDlpCandidate>> SearchAsync(string q, int count, CancellationToken ct) => Task.FromResult(Entries);
        public Task<IReadOnlyList<YtDlpCandidate>> PlaylistEntriesAsync(string url, int max, CancellationToken ct)
        {
            MixUrl = url;
            return Task.FromResult(Entries);
        }
    }

    private static PlayableRef Seed(string url)
        => new(SourceKind.YouTube, url, true, new TrackInfo("t", "a", SourceKind.YouTube, url, 1000, null, false, 42));

    [Fact]
    public async Task Builds_RD_mix_and_filters_out_the_seed()
    {
        var fake = new FakeYtDlp
        {
            Entries = new List<YtDlpCandidate>
            {
                new("abc", "seed", "c", 1000, "https://youtu.be/abc"),
                new("def", "rel1", "c", 1000, "https://www.youtube.com/watch?v=def"),
            },
        };
        var provider = new YouTubeAutoplayProvider(fake);

        var result = await provider.RelatedAsync(Seed("https://www.youtube.com/watch?v=abc"), 10, default);

        Assert.Equal("https://www.youtube.com/watch?v=abc&list=RDabc", fake.MixUrl);
        Assert.Single(result);
        Assert.Equal("rel1", result[0].Info.Title);
        Assert.Equal(42ul, result[0].Info.RequestedBy);
        Assert.True(result[0].NeedsResolution);
    }

    [Fact]
    public async Task Non_youtube_seed_returns_empty()
    {
        var provider = new YouTubeAutoplayProvider(new FakeYtDlp());
        var seed = new PlayableRef(SourceKind.DirectUrl, "http://x/a.mp3", false,
            new TrackInfo("t", "a", SourceKind.DirectUrl, "http://x/a.mp3", 0, null, false, 1));

        Assert.Empty(await provider.RelatedAsync(seed, 10, default));
    }

    [Fact]
    public async Task Short_youtu_be_seed_extracts_id()
    {
        var fake = new FakeYtDlp();
        var provider = new YouTubeAutoplayProvider(fake);

        await provider.RelatedAsync(Seed("https://youtu.be/xyz"), 5, default);

        Assert.Equal("https://www.youtube.com/watch?v=xyz&list=RDxyz", fake.MixUrl);
    }
}
