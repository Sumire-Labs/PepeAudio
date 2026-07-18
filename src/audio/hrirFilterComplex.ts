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
 *
 * `correctiveEq` (optional) is spliced in BEFORE the makeup gain + limiter: a
 * per-IR tone-correction chain (low-shelf / presence-peak / high-shelf) measured
 * at load (see config/hrirProfiles.measureCorrectiveEq). It flattens the BRIR's
 * "front speaker in a room" coloration — convolving finished stereo through a
 * BRIR pushes the CENTER content (bass/kick/lead vocal, which is where a mix's
 * perceived tone lives) through only the front-speaker IRs, whose boomy-bass /
 * recessed-highs response is the audible "低音が強すぎ / 高音がスカスカ" defect.
 * Placed before the limiter on purpose: taming the bass hump first means the
 * limiter no longer rides on over-hot low end (which was crushing bass
 * transients). Empty string = no measurable tilt for this IR (or measurement
 * failed) → no-op, identical to the pre-correction chain.
 */
function makeupTail(makeupDb: number, correctiveEq: string): string {
  const eq = correctiveEq ? `,${correctiveEq}` : '';
  return `${eq},volume=${makeupDb.toFixed(2)}dB,alimiter=limit=0.95`;
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
function simpleFilterComplex(makeupDb: number, aura360Prefix: string, correctiveEq: string): string {
  if (aura360Prefix) {
    return `[1:a]aresample=48000[ir];[0:a]${aura360Prefix}[pre];[pre][ir]afir${makeupTail(makeupDb, correctiveEq)}[out]`;
  }
  return `[1:a]aresample=48000[ir];[0:a][ir]afir${makeupTail(makeupDb, correctiveEq)}[out]`;
}

/**
 * `-filter_complex` for a genuine HeSuVi-style 14-channel HRIR file (format:
 * 'hesuvi14'). HeSuVi's own files encode one impulse response per
 * (virtual speaker, ear) pair across up to 7 speakers (14 = 7 × 2 ears) for
 * a full 7.1 virtualizer. This bot has 2-channel (stereo music) input, so
 * rather than a naive upmix (a stereo→7.1 `surround` upmix was measured to
 * re-correlate the two ears and NARROW the image toward mono), it drives the
 * FRONT speakers with the direct L/R and the SIDE speakers with the
 * decorrelated L−R "difference" (ambience) signal — a Pro-Logic-II-"Music"-
 * style routing that genuinely widens/externalises the image. See the function
 * body for the exact 8-lane feed; front + side pairs are used (not just fronts).
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
function hesuvi14FilterComplex(makeupDb: number, aura360Prefix: string, correctiveEq: string): string {
  const pre = aura360Prefix ? `${aura360Prefix},` : '';
  // Virtual surround using the FRONT and SIDE speaker pairs of the HeSuVi BRIR
  // (channel indices: FL 0/1, FR 8/7, SL 2/3, SR 10/9 — note the right-side
  // ear-order flip). The 8-lane afir feed drives:
  //   - fronts with the direct L / R  (keeps vocals/centre intimate), and
  //   - the side speakers with the L−R "difference" (ambience) signal, which is
  //     decorrelated from the fronts — THAT is what pushes the image out of the
  //     head and gives 立体感 (measured: interaural correlation drops below the
  //     source, mono-downmix widens, HF preserved, mono-safe: the side lanes
  //     null to silence when L==R so nothing cancels on phone speakers).
  // irlink=true is load-bearing — it keeps each speaker's L/R-ear IR pair
  // independent; without it afir averages the ears and the spatial image
  // collapses. A naive stereo→7.1 `surround` upmix was tried and REJECTED: it
  // re-correlates the two ears and narrows toward mono, the opposite of wrap.
  return (
    '[1:a]aresample=48000[ir];' +
    '[ir]pan=8c|c0=c0|c1=c1|c2=c8|c3=c7|c4=c2|c5=c3|c6=c10|c7=c9[ir8];' +
    `[0:a]${pre}pan=8c|c0=c0|c1=c0|c2=c1|c3=c1|c4=0.5*c0-0.5*c1|c5=0.5*c0-0.5*c1|c6=0.5*c1-0.5*c0|c7=0.5*c1-0.5*c0[feed];` +
    '[feed][ir8]afir=irfmt=input:irlink=true[conv];' +
    `[conv]pan=stereo|c0=c0+c2+c4+c6|c1=c1+c3+c5+c7${makeupTail(makeupDb, correctiveEq)}[out]`
  );
}

/**
 * Builds the `-filter_complex` graph (mapped to `[out]` by the caller) for the
 * afir BRIR path, with the per-IR `makeupDb` baked into the level-match tail.
 * Both formats output a level-matched stereo signal at 48 kHz.
 */
export function buildHrirFilterComplex(
  format: HrirFormat,
  makeupDb: number,
  aura360Prefix = '',
  correctiveEq = '',
): string {
  return format === 'hesuvi14'
    ? hesuvi14FilterComplex(makeupDb, aura360Prefix, correctiveEq)
    : simpleFilterComplex(makeupDb, aura360Prefix, correctiveEq);
}

/**
 * The afir BRIR chain at unity makeup, with an `astats` probe spliced in before
 * the `[out]` sink. Used ONLY by config/hrirProfiles.measureHrirMakeupDb at
 * startup to measure the convolution's natural level loss against a reference
 * signal — running the REAL production graph (rather than a hand-rolled proxy)
 * guarantees the measured makeup matches what actually plays. At unity the
 * limiter never engages on the ~-48 dB raw output, so it doesn't skew the
 * reading.
 *
 * `correctiveEq` is passed through so makeup is measured on the SAME graph that
 * plays (EQ then makeup): the corrective shelves/peak shift broadband level a
 * little, and measuring with them baked in keeps the level-match exact instead
 * of off by the EQ's net gain.
 */
export function buildHrirMeasureGraph(format: HrirFormat, correctiveEq = ''): string {
  return buildHrirFilterComplex(format, 0, '', correctiveEq).replace(/\[out\]$/, ',astats=metadata=0[out]');
}
