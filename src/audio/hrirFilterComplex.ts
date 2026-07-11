import type { HrirFormat } from '../config/hrirProfiles.js';

/**
 * Level-match tail appended to every afir BRIR chain.
 *
 * afir applies NO makeup gain of its own, and convolving a finished stereo
 * master through a room BRIR drops broadband level by ~20 dB (measured: the
 * bundled Dolby Home Theater V4 file lands ~20 dB below the pristine
 * passthrough with no compensation). That quiet, "distant" output was the real
 * defect behind the old spatial mode sounding weak/low-fi — NOT the BRIR's tone
 * (its spectral tilt matches the source within ~0.3 dB). So the fix is a single
 * measured makeup gain, computed PER IR at load (see
 * config/hrirProfiles.measureHrirMakeupDb) so any bring-your-own BRIR is
 * auto-levelled, not a value hardcoded to one file.
 *
 * `alimiter` is a transparent safety net: at the level-matched operating point
 * it barely engages (measured peak ~-16 dBFS, well under the 0.95 threshold),
 * but it catches the occasional already-hot master that would otherwise clip
 * the s16le output after makeup. This is deliberately ONE gain + one limiter,
 * not the previous over-processed loudness/limiter/bass-split stack that was
 * rolled back for making things worse.
 */
function makeupTail(makeupDb: number): string {
  return `,volume=${makeupDb.toFixed(2)}dB,alimiter=limit=0.95`;
}

/**
 * `-filter_complex` for a plain mono/stereo bring-your-own HRIR file (see
 * config/hrirProfiles.ts, format: 'simple'). Unlike sofalizer's `sofa=<path>`
 * filter *option*, afir takes its IR from a second real ffmpeg *input*
 * ("[1:a]") referenced only by index here — so there's no path text embedded
 * in this string, and none of the drive-letter-colon escaping problem applies.
 * Input 1 is explicitly resampled to 48000 first: it's a user-supplied file of
 * unknown sample rate, and a mismatched rate would otherwise convolve at the
 * wrong effective pitch/speed. Caller is responsible for adding the IR file as
 * ffmpeg input index 1 (`-i <path>`, after the main `pipe:0` input).
 */
function simpleFilterComplex(makeupDb: number): string {
  return `[1:a]aresample=48000[ir];[0:a][ir]afir${makeupTail(makeupDb)}[out]`;
}

/**
 * `-filter_complex` for a genuine HeSuVi-style 14-channel HRIR file (format:
 * 'hesuvi14'). HeSuVi's own files encode one impulse response per
 * (virtual speaker, ear) pair across up to 7 speakers (14 = 7 × 2 ears) for
 * a full 7.1 virtualizer — but this bot only ever has 2-channel (stereo
 * music) input, so there is no real content for the center/side/back
 * speakers; convolving them would just be processing silence. This instead
 * does "true stereo" processing using only the front-left/front-right pair,
 * matching ffmpeg's own documented afir true-stereo recipe.
 *
 * Channel indices (0-based) were reverse-engineered against a real HeSuVi
 * install (verified 2026-07-11 against actual atmos.wav and dht.wav files by
 * feeding hard-left/hard-right test tones through and confirming distinct,
 * comparable-level, non-silent output on both sides):
 *   - channel 0 = front-left speaker's response as heard by the LEFT ear
 *   - channel 1 = front-left speaker's response as heard by the RIGHT ear
 *   - channel 7 = front-right speaker's response as heard by the RIGHT ear
 *   - channel 8 = front-right speaker's response as heard by the LEFT ear
 * (A community-published channel-mapping reference for the same 14-channel
 * format lists the same four indices for this speaker pair; this was cross-
 * checked against that source, then independently confirmed against the
 * real files before shipping.)
 *
 * The two `pan` extractions both read from the same upstream (resampled)
 * `[1:a]`, which ffmpeg requires an explicit `asplit` for — a labeled filter
 * output can only feed ONE downstream filter, and reusing it un-split doesn't
 * error, it just silently starves the second consumer (confirmed directly:
 * omitting asplit here produced correct front-left output but near-total
 * silence from the front-right side).
 */
function hesuvi14FilterComplex(makeupDb: number): string {
  return (
    '[1:a]aresample=48000,asplit=2[ir14a][ir14b];' +
    '[ir14a]pan=stereo|c0=c0|c1=c1[LIR];' +
    '[ir14b]pan=stereo|c0=c8|c1=c7[RIR];' +
    '[0:a]pan=4c|c0=FL|c1=FL|c2=FR|c3=FR[a];' +
    '[LIR][RIR]amerge=inputs=2[ir4];' +
    '[a][ir4]afir=irfmt=input[or];' +
    `[or]pan=stereo|FL<c0+c2|FR<c1+c3${makeupTail(makeupDb)}[out]`
  );
}

/**
 * Builds the `-filter_complex` graph (mapped to `[out]` by the caller) for the
 * afir BRIR path, with the per-IR `makeupDb` baked into the level-match tail.
 * Both formats output a level-matched stereo signal at 48 kHz.
 */
export function buildHrirFilterComplex(format: HrirFormat, makeupDb: number): string {
  return format === 'hesuvi14' ? hesuvi14FilterComplex(makeupDb) : simpleFilterComplex(makeupDb);
}

/**
 * The afir BRIR chain at unity makeup, with an `astats` probe spliced in before
 * the `[out]` sink. Used ONLY by config/hrirProfiles.measureHrirMakeupDb at
 * startup to measure the convolution's natural level loss against a reference
 * signal — running the REAL production graph (rather than a hand-rolled proxy)
 * guarantees the measured makeup matches what actually plays. At unity the
 * limiter never engages on the ~-48 dB raw output, so it doesn't skew the
 * reading.
 */
export function buildHrirMeasureGraph(format: HrirFormat): string {
  return buildHrirFilterComplex(format, 0).replace(/\[out\]$/, ',astats=metadata=0[out]');
}
