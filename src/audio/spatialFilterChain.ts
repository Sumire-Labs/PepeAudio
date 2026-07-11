/**
 * The `-af` chain used for "360° Sound" ON when NO bring-your-own BRIR file is
 * available (see config/hrirProfiles.ts) — the universal, asset-free fallback.
 *
 * The premium path is the afir BRIR convolution (audio/hrirFilterComplex.ts),
 * which needs a HeSuVi/BRIR file the project doesn't bundle. Where none is
 * present, this delivers a lighter static "wider, out-of-head" feel using only
 * base ffmpeg filters, so it works everywhere — including the guaranteed
 * ffmpeg-static fallback binary, which has NO sofalizer/libmysofa.
 *
 * Deliberately spectrally gentle and STATIC (no rotation): the old sofalizer
 * (raw MIT KEMAR HRTF) and the old `earwax`-based lite chain both convolved a
 * fixed HRTF that rolled off the top octaves and comb-filtered the spectrum —
 * the exact "muffled / cheap-earphone" defect this rework removes. This chain
 * never convolves an HRTF:
 *   - `crossfeed`  — mild headphone crossfeed; glues the stereo image so it
 *                    sits outside the head instead of hard-panned in each ear.
 *                    Touches mainly the low end, leaving highs intact.
 *   - `stereotools=slev` — a modest mid/side SIDE-level lift for width, chosen
 *                    over the harsh `extrastereo` so transients don't smear.
 *   - `volume=2dB` — level-matches the crossfeed's small loss back to the
 *                    pristine passthrough (measured), so toggling isn't jarring.
 *   - `alimiter`   — transparent clip guard for the occasional hot master.
 * Measured: full-band (HF preserved vs pristine), mono-safe (~2 dB downmix
 * drop, no cancellation), no clipping.
 */
const SPATIAL_FALLBACK_CHAIN = 'crossfeed=strength=0.35:range=0.6,stereotools=slev=1.25,volume=2dB,alimiter=limit=0.97';

/** Returns the `-af` fallback spatial chain used when 360° is on but no BRIR file is configured. */
export function buildSpatialFallbackChain(): string {
  return SPATIAL_FALLBACK_CHAIN;
}
