/**
 * `-filter_complex` for a plain mono/stereo bring-your-own HRIR file (see
 * config/hrirProfiles.ts, format: 'simple'). Unlike sofalizer's `sofa=<path>`
 * filter *option*, afir takes its IR from a second real ffmpeg *input*
 * ("[1:a]") referenced only by index here — so there's no path text embedded
 * in this string, and none of sofalizer's drive-letter-colon escaping problem
 * applies. Input 1 is explicitly resampled to 48000 first: it's a
 * user-supplied file of unknown sample rate, and a mismatched rate would
 * otherwise convolve at the wrong effective pitch/speed. Caller is
 * responsible for adding the IR file as ffmpeg input index 1 (`-i <path>`,
 * after the main `pipe:0` input).
 */
export const HRIR_SIMPLE_FILTER_COMPLEX = '[1:a]aresample=48000[ir];[0:a][ir]afir[out]';

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
export const HRIR_HESUVI14_FILTER_COMPLEX =
  '[1:a]aresample=48000,asplit=2[ir14a][ir14b];' +
  '[ir14a]pan=stereo|c0=c0|c1=c1[LIR];' +
  '[ir14b]pan=stereo|c0=c8|c1=c7[RIR];' +
  '[0:a]pan=4c|c0=FL|c1=FL|c2=FR|c3=FR[a];' +
  '[LIR][RIR]amerge=inputs=2[ir4];' +
  '[a][ir4]afir=irfmt=input[outraw];' +
  '[outraw]pan=stereo|FL<c0+c2|FR<c1+c3[out]';
