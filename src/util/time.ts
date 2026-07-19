// Some sources report 0 (not null) for live/unknown duration; treat 0 as live too.
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
