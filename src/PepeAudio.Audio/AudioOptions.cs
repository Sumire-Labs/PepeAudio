// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Audio;

public sealed class AudioOptions
{
    public const string Section = "Audio";

    public string FFmpegPath { get; set; } = "ffmpeg";
    public string PresetsDir { get; set; } = "assets/audio/hesuvi";
    public string DefaultPreset { get; set; } = "Aura";
    public int MaxConcurrentVoices { get; set; } = 50;
    public int PcmPrebufferMs { get; set; } = 300;
    public int DiscordPcmBufferMs { get; set; } = 200;
}
