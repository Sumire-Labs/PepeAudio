/**
 * Per QueueItem's contract, durationMs should be null for live/unknown-duration
 * content, but some source metadata (YouTube livestreams in progress, etc.)
 * reports 0 instead — treat that the same as null everywhere duration is used,
 * rather than each call site deciding independently (previously this file and
 * progressBar.ts disagreed: one showed "LIVE", the other showed "0:00").
 */
export function isLiveDuration(ms: number | null): boolean {
  return ms === null || !Number.isFinite(ms) || ms <= 0;
}

export function formatDuration(ms: number | null): string {
  if (isLiveDuration(ms)) return 'LIVE';
  const totalSeconds = Math.max(0, Math.floor((ms as number) / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const ss = String(seconds).padStart(2, '0');
  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, '0')}:${ss}`;
  }
  return `${minutes}:${ss}`;
}
