import { PROGRESS_BAR_WIDTH } from '../player/constants.js';
import { isLiveDuration } from '../util/time.js';

export function renderProgressBar(elapsedMs: number, durationMs: number | null, width = PROGRESS_BAR_WIDTH): string {
  if (durationMs === null || isLiveDuration(durationMs)) {
    return '🔴 LIVE ' + '░'.repeat(width);
  }
  const ratio = Math.min(1, Math.max(0, elapsedMs / durationMs));
  const filled = Math.min(width, Math.round(ratio * width));
  return '█'.repeat(filled) + '░'.repeat(width - filled);
}
