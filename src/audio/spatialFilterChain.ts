/**
 * `-af` chain for Aura HRIR ON with no bring-your-own BRIR file — the asset-free
 * fallback. Works everywhere, including the ffmpeg-static binary which has NO
 * sofalizer/libmysofa. Deliberately STATIC and spectrally gentle: never convolves
 * an HRTF, avoiding the "muffled" defect the old sofalizer/earwax chains produced.
 * `volume=2dB` level-matches the crossfeed loss so toggling on/off isn't jarring.
 */
const HRIR_FALLBACK_CHAIN = 'crossfeed=strength=0.35:range=0.6,stereotools=slev=1.25,volume=2dB,alimiter=limit=0.97';

/** Fallback `-af` chain: Aura HRIR on but no BRIR file configured. */
export function buildHrirFallbackChain(): string {
  return HRIR_FALLBACK_CHAIN;
}

/**
 * Aura 360° — a separate, independently-toggleable feature from Aura HRIR; does
 * NOT convolve/externalise. Gotcha: aecho `out_gain` scales the whole echo mix
 * (not just the wet), so the trailing `volume` re-levels it back toward unity.
 */
const AURA360_CORE = 'stereotools=slev=1.4,bass=g=4:f=100,aecho=in_gain=1:out_gain=0.82:delays=28|55:decays=0.38|0.26,volume=-1dB';

/** Aura 360° filters WITHOUT a limiter — prepended into the afir graph when Aura HRIR is also on (afir's own alimiter is the safety net). */
export function buildAura360Prefix(): string {
  return AURA360_CORE;
}

/** Aura 360° as a standalone `-af` chain (Aura 360° on, Aura HRIR off), with its own safety limiter. */
export function buildAura360Chain(): string {
  return `${AURA360_CORE},alimiter=limit=0.97`;
}
