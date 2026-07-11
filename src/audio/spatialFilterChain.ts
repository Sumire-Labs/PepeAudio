import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { logger } from '../logger.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(__dirname, '..', '..');

/** MIT Media Lab KEMAR HRTF dataset — see assets/NOTICE.md for licensing/attribution. */
export const HRTF_SOFA_PATH = path.join(PROJECT_ROOT, 'assets', 'hrtf', 'mit_kemar_normal_pinna.sofa');

/** Lightweight, asset-free spatializer chain used when true HRTF convolution isn't available. */
const LITE_CHAIN = 'earwax,stereowiden=delay=20:feedback=0.3:crossfeed=0.3:drymix=0.8,extrastereo=m=1.5';

/**
 * ffmpeg's filtergraph parser splits a filter's options on unescaped `:`, and its
 * escaping is two-pass (graph-level then option-level) — a Windows drive-letter
 * colon (`C:/...`) needs *doubled* backslash-escaping to survive both passes,
 * which is exactly as fragile as it sounds (confirmed by hitting the literal
 * "No option name near ..." parser error with single-escaping). A path *relative*
 * to the spawned ffmpeg process's cwd has no colon at all, sidestepping the
 * escaping problem entirely — and since we spawn ffmpeg ourselves with no cwd
 * override, computing the relative path against `process.cwd()` is always
 * correct, whatever directory the bot is actually run from.
 */
function toFfmpegSafePath(absolutePath: string): string | null {
  const relative = path.relative(process.cwd(), absolutePath);
  const normalized = relative.split(path.sep).join('/');
  if (normalized.includes(':')) {
    // Only happens if cwd and the target are on different drives (Windows) —
    // path.relative() can't express that as a relative path.
    return null;
  }
  return normalized;
}

/**
 * Builds the `-af` filter chain for the spatial-audio path.
 * Deliberately bare: just sofalizer with its own ffmpeg defaults (no gain
 * boost, no bass-management split, no EQ/loudness/limiter stages) — all of
 * that was tried and rolled back at the user's request after it made things
 * sound worse (over-processed/muffled), not better. If sofalizer's own
 * defaults need adjusting, do it deliberately and test one change at a time
 * rather than stacking several unverified-by-ear tweaks again.
 * Falls back to the lite chain both when the ffmpeg build lacks libmysofa
 * (`useSofalizer=false`) and when the HRTF file's path can't be made
 * ffmpeg-safe (see toFfmpegSafePath).
 */
export function buildFilterChain(useSofalizer: boolean): string {
  if (useSofalizer) {
    const safePath = toFfmpegSafePath(HRTF_SOFA_PATH);
    if (safePath) {
      return `sofalizer=sofa=${safePath}`;
    }
    logger.warn(
      { hrtfPath: HRTF_SOFA_PATH, cwd: process.cwd() },
      'Could not build an ffmpeg-safe relative path to the HRTF file (different drive than cwd?) - using the lightweight spatial chain instead',
    );
  }
  return LITE_CHAIN;
}
