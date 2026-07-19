import type { HrirFormat } from '../config/hrirProfiles.js';

/**
 * Level-match tail for every afir BRIR chain. afir applies NO makeup gain, and
 * convolving a stereo master through a room BRIR drops broadband level ~20 dB, so
 * `makeupDb` is measured PER IR at load (config/hrirProfiles.measureHrirMakeupDb).
 * `alimiter` is a safety net for already-hot masters that would clip s16le after makeup.
 */
function makeupTail(makeupDb: number): string {
  return `,volume=${makeupDb.toFixed(2)}dB,alimiter=limit=0.95`;
}

/**
 * `-filter_complex` for a simple mono/stereo HRIR file (format: 'simple'). afir
 * takes its IR from ffmpeg input index 1 (`[1:a]`), so the caller MUST add the IR
 * file as `-i` after the main `pipe:0` input. Input 1 is resampled to 48000 first:
 * an unknown-rate user file would otherwise convolve at the wrong pitch/speed.
 */
function simpleFilterComplex(makeupDb: number, aura360Prefix: string): string {
  if (aura360Prefix) {
    return `[1:a]aresample=48000[ir];[0:a]${aura360Prefix}[pre];[pre][ir]afir${makeupTail(makeupDb)}[out]`;
  }
  return `[1:a]aresample=48000[ir];[0:a][ir]afir${makeupTail(makeupDb)}[out]`;
}

/**
 * `-filter_complex` for a HeSuVi-style 14-channel HRIR file (format: 'hesuvi14',
 * 7 virtual speakers × 2 ears). Input is stereo, so instead of a naive stereo→7.1
 * `surround` upmix (which re-correlates the ears and NARROWS toward mono) the
 * fronts get direct L/R and the sides get the decorrelated L−R difference
 * (Pro-Logic-II-Music-style) to widen/externalise the image. Channel indices were
 * reverse-engineered against real HeSuVi files (verified 2026-07-11).
 */
function hesuvi14FilterComplex(makeupDb: number, aura360Prefix: string): string {
  const pre = aura360Prefix ? `${aura360Prefix},` : '';
  // FRONT + SIDE speaker pairs (channel indices: FL 0/1, FR 8/7, SL 2/3,
  // SR 10/9 — note the right-side ear-order flip). Fronts get direct L/R; sides
  // get the decorrelated L−R difference. Mono-safe: side lanes null when L==R,
  // so nothing cancels on phone speakers.
  // irlink=true is load-bearing: it keeps each speaker's L/R-ear IR pair
  // independent; without it afir averages the ears and the spatial image collapses.
  return (
    '[1:a]aresample=48000[ir];' +
    '[ir]pan=8c|c0=c0|c1=c1|c2=c8|c3=c7|c4=c2|c5=c3|c6=c10|c7=c9[ir8];' +
    `[0:a]${pre}pan=8c|c0=c0|c1=c0|c2=c1|c3=c1|c4=0.5*c0-0.5*c1|c5=0.5*c0-0.5*c1|c6=0.5*c1-0.5*c0|c7=0.5*c1-0.5*c0[feed];` +
    '[feed][ir8]afir=irfmt=input:irlink=true[conv];' +
    `[conv]pan=stereo|c0=c0+c2+c4+c6|c1=c1+c3+c5+c7${makeupTail(makeupDb)}[out]`
  );
}

/** Builds the afir BRIR `-filter_complex` graph (caller maps `[out]`), with per-IR `makeupDb` baked into the level-match tail. */
export function buildHrirFilterComplex(format: HrirFormat, makeupDb: number, aura360Prefix = ''): string {
  return format === 'hesuvi14'
    ? hesuvi14FilterComplex(makeupDb, aura360Prefix)
    : simpleFilterComplex(makeupDb, aura360Prefix);
}

/**
 * afir BRIR chain at unity makeup with an `astats` probe before `[out]`. Used
 * ONLY by config/hrirProfiles.measureHrirMakeupDb to measure the convolution's
 * natural level loss — reusing the REAL production graph keeps the measured makeup
 * matched to what plays. At unity the limiter never engages, so it doesn't skew it.
 */
export function buildHrirMeasureGraph(format: HrirFormat): string {
  return buildHrirFilterComplex(format, 0).replace(/\[out\]$/, ',astats=metadata=0[out]');
}
