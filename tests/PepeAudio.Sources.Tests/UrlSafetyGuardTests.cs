// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Sources.Security;
using Xunit;

namespace PepeAudio.Sources.Tests;

public class UrlSafetyGuardTests
{
    [Theory]
    [InlineData("https://example.com/song.mp3", true)]
    [InlineData("http://example.com/song.mp3", false)]   // http not allowed
    [InlineData("https://127.0.0.1/x", false)]           // loopback
    [InlineData("https://10.0.0.5/x", false)]            // private
    [InlineData("https://169.254.169.254/latest", false)] // cloud metadata
    [InlineData("ftp://example.com/x", false)]
    [InlineData("not a url", false)]
    public void Validates_scheme_and_blocks_private(string url, bool expected)
        => Assert.Equal(expected, UrlSafetyGuard.IsSafeHttpUrl(url, out _));
}
