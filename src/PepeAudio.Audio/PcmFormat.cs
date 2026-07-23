// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Audio;

// Discord voice PCM format: 48kHz, 16-bit, stereo, 20ms frames.
public static class PcmFormat
{
    public const int SampleRate = 48000;
    public const int Channels = 2;
    public const int BytesPerSample = 2;
    public const int FrameMs = 20;
    public const int FrameBytes = SampleRate / 1000 * FrameMs * Channels * BytesPerSample; // 3840
}
