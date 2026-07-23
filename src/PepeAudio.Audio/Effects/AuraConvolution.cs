// SPDX-License-Identifier: Apache-2.0
using System.Globalization;

namespace PepeAudio.Audio.Effects;

// Builds the FFmpeg convolution filtergraph for a HeSuVi preset.
// 14ch = full 7.0 HeSuVi BRIR (immersive, headphone filter); 4ch = true-stereo afir; 2ch = plain stereo IR.
public static class AuraConvolution
{
    private const string Afir = "afir=irfmt=input:minp=1024";
    // Shared 48k soxr resample stage — the input to both the live chain and the measure graph.
    internal const string SoxrResample = "aresample=48000:resampler=soxr:precision=28";
    // level=false: alimiter defaults to auto-level (scales output by 1/limit), which would cancel
    // the 0.95 headroom the limit is there to provide. latency=1 compensates the lookahead delay.
    internal const string Limiter = "alimiter=limit=0.95:level=false:latency=1";

    // Aura 360° tone stage: punch from a 60 Hz peaking EQ (the TS shelf at 100 Hz also lifted
    // 150-250 Hz and read as boomy), a gentle air shelf against BRIR darkening, and a trim that
    // keeps the stage ≈level-neutral. Convolved paths re-measure makeup WITH this stage anyway.
    internal const string Aura360Tone = "equalizer=f=60:width_type=o:w=1.1:g=3.5,treble=g=1.5:f=9500,volume=-1dB";
    // Aura 360° spatial stage for paths WITHOUT rear BRIRs: Haas cross-delay decorrelation
    // (stereowiden) gives true interaural width/depth — unlike TS's aecho, whose identical
    // per-channel echoes stayed ear-correlated and combed — then crossfeed pulls the lows back
    // to centre so bass stays solid and mono-safe despite the widening.
    internal const string Aura360Widen =
        "stereowiden=delay=16:feedback=0.25:crossfeed=0.35:drymix=0.9,crossfeed=strength=0.4:range=0.5,volume=-0.5dB";
    // The measure graph (see BuildMeasureGraph) reuses SoxrResample with no normalization/makeup,
    // ending in an astats probe.
    private const string MeasureOut = "aformat=channel_layouts=stereo,astats=metadata=0";

    public static string Build(HeSuViPreset preset, string pre, string outFormat, bool aura360 = false)
    {
        // The soxr stage after amovie pins the IR branch to 48k; without it a non-48k IR would be
        // auto-resampled by the default swr (or, pre-negotiation, drag the whole graph off 48k).
        var ir = $"amovie={EscapePath(preset.Path)},{SoxrResample}";
        // BRIR convolution drops broadband level a lot; makeup (measured per preset at load) restores
        // it to match bypass, and the limiter caps peaks so a hot master can't clip s16le after the boost.
        var makeup = MakeupTail(aura360 ? preset.Makeup360Db : preset.MakeupDb);
        // 14ch presets render 360° through the BRIR's real rear speakers (Immersive6); other
        // layouts have no rear IRs, so 360° falls back to the Haas widen stage before convolving.
        if (aura360) pre = $"{pre},{Aura360Tone}" + (preset.Channels == 14 ? "" : $",{Aura360Widen}");
        return preset.Channels switch
        {
            14 => aura360 ? Immersive6(ir, pre, makeup, outFormat) : Immersive(ir, pre, makeup, outFormat),
            4 => $"{pre},pan=4C|c0=FL|c1=FL|c2=FR|c3=FR[a];{ir}[ir];" +
                 $"[a][ir]{Afir}[b];[b]pan=stereo|FL=c0+c2|FR=c1+c3,{makeup},{outFormat}[out]",
            2 => $"{pre}[a];{ir}[ir];[a][ir]{Afir},{makeup},{outFormat}[out]",
            _ => $"{pre},{outFormat}[out]",
        };
    }

    // Exactly the production convolution at 0 dB makeup, ending in astats, so the loader can read the
    // broadband level the graph loses and turn it into this preset's makeup (see HeSuViPresetLibrary).
    public static string BuildMeasureGraph(HeSuViPreset preset, bool aura360 = false)
        => Build(preset with { MakeupDb = 0, Makeup360Db = 0 }, SoxrResample, MeasureOut, aura360);

    private static string MakeupTail(double db)
        => $"volume={db.ToString("0.##", CultureInfo.InvariantCulture)}dB,{Limiter}";

    // 4 virtual speakers — front L/R (±30°) + sides (±90°), no centre/back (the old bot's layout,
    // which images wider and crisper for stereo music than a full 7.0 bed). Fronts carry the direct
    // L/R; sides get the full-band L/R difference (ambience/width), prominent and undelayed so the
    // image is wide and out-of-head. headphone(hrir=stereo) convolves each speaker's real
    // (left-ear,right-ear) IR pair (sliced BY INDEX from HeSuVi's native 14ch order — right speakers
    // stored right-ear-first) and sums to stereo. Per-preset makeup re-balances total loudness.
    private static string Immersive(string ir, string pre, string makeup, string outFormat)
    {
        return
            $"{pre},asplit=4[mFL][mFR][mSL][mSR];" +
            "[mFL]pan=mono|c0=c0[Lf];[mFR]pan=mono|c0=c1[Rf];" +
            "[mSL]pan=mono|c0=c0-0.5*c1,volume=-1dB[SL];" +
            "[mSR]pan=mono|c0=c1-0.5*c0,volume=-1dB[SR];" +
            "[Lf][Rf][SL][SR]join=inputs=4:channel_layout=FL+FR+SL+SR:map=0.0-FL|1.0-FR|2.0-SL|3.0-SR[spk];" +
            $"{ir},asplit=4[i0][i1][i2][i3];" +
            "[i0]pan=stereo|c0=c0|c1=c1[hFL];[i1]pan=stereo|c0=c8|c1=c7[hFR];" +
            "[i2]pan=stereo|c0=c2|c1=c3[hSL];[i3]pan=stereo|c0=c10|c1=c9[hSR];" +
            "[spk][hFL][hFR][hSL][hSR]headphone=map=FL|FR|SL|SR:hrir=stereo:type=freq[bin];" +
            $"[bin]{makeup},{outFormat}[out]";
    }

    // Aura 360° layout: Immersive plus two rear virtual speakers rendered through the BRIR's REAL
    // back-speaker pairs (HeSuVi BL=c4/c5, BR=c12(L-ear)/c11(R-ear)) — measured rear-hemisphere cues
    // the TS enhancer's aecho could only fake. Rears carry the same difference/ambience signal as
    // the sides, but delayed (asymmetric 15/21 ms so the rear field decorrelates into diffuseness
    // instead of a phantom rear-centre), darkened (air absorption) and attenuated, which pushes
    // them perceptually behind and away from the listener.
    private static string Immersive6(string ir, string pre, string makeup, string outFormat)
    {
        return
            $"{pre},asplit=6[mFL][mFR][mSL][mSR][mBL][mBR];" +
            "[mFL]pan=mono|c0=c0[Lf];[mFR]pan=mono|c0=c1[Rf];" +
            "[mSL]pan=mono|c0=c0-0.5*c1,volume=-1dB[SL];" +
            "[mSR]pan=mono|c0=c1-0.5*c0,volume=-1dB[SR];" +
            "[mBL]pan=mono|c0=c0-0.5*c1,adelay=15:all=1,lowpass=f=6500,volume=-3dB[BL];" +
            "[mBR]pan=mono|c0=c1-0.5*c0,adelay=21:all=1,lowpass=f=6500,volume=-3dB[BR];" +
            "[Lf][Rf][SL][SR][BL][BR]join=inputs=6:channel_layout=FL+FR+SL+SR+BL+BR:" +
            "map=0.0-FL|1.0-FR|2.0-SL|3.0-SR|4.0-BL|5.0-BR[spk];" +
            $"{ir},asplit=6[i0][i1][i2][i3][i4][i5];" +
            "[i0]pan=stereo|c0=c0|c1=c1[hFL];[i1]pan=stereo|c0=c8|c1=c7[hFR];" +
            "[i2]pan=stereo|c0=c2|c1=c3[hSL];[i3]pan=stereo|c0=c10|c1=c9[hSR];" +
            "[i4]pan=stereo|c0=c4|c1=c5[hBL];[i5]pan=stereo|c0=c12|c1=c11[hBR];" +
            "[spk][hFL][hFR][hSL][hSR][hBL][hBR]headphone=map=FL|FR|SL|SR|BL|BR:hrir=stereo:type=freq[bin];" +
            $"[bin]{makeup},{outFormat}[out]";
    }

    // amovie filename sits inside a filter option, so filtergraph-special chars (':' from a Windows
    // drive letter, plus ,;[]' in odd filenames) need the two-level escape '\\X' — one level is
    // consumed by the graph parser, the other by the option parser.
    private static string EscapePath(string path)
    {
        var p = path.Replace('\\', '/');
        foreach (var c in ":,;[]'")
            p = p.Replace(c.ToString(), @"\\" + c);
        return p;
    }
}
