// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Audio.Effects;

// A HeSuVi impulse response. Channels: 2 = plain stereo IR, 4 = true-stereo
// (L->L, L->R, R->L, R->R), 14 = standard HeSuVi 7.1 BRIR (7 speaker pairs).
// MakeupDb is the loudness compensation, measured per preset at load (see HeSuViPresetLibrary).
public sealed record HeSuViPreset(string Name, string Path, int Channels, double MakeupDb = 16)
{
    public bool IsSupported => Channels is 2 or 4 or 14;
}
