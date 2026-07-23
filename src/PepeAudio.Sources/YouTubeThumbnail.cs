// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Sources;

// hqdefault.jpg exists for every video (maxresdefault often 404s), so it always renders in Discord.
public static class YouTubeThumbnail
{
    public static string For(string videoId) => $"https://i.ytimg.com/vi/{videoId}/hqdefault.jpg";
}
