import type { PlayerSnapshot } from "./types";

export const ORBIT_PERIOD_MS = 60_000;

export function interpolatedPositionMs(
  snapshot: PlayerSnapshot,
  nowUnixMs: number
): number {
  const track = snapshot.track;
  if (track === null) {
    return 0;
  }

  const elapsed =
    snapshot.state === "playing"
      ? Math.max(0, nowUnixMs - track.anchorUnixMs)
      : 0;
  const position = track.positionMsAtAnchor + elapsed;
  return track.durationMs === null
    ? Math.max(0, position)
    : Math.min(Math.max(0, position), track.durationMs);
}

export function orbitDegreesAt(snapshot: PlayerSnapshot, nowUnixMs: number): number {
  const progress = interpolatedPositionMs(snapshot, nowUnixMs) % ORBIT_PERIOD_MS;
  return (snapshot.orbitDegrees + (progress / ORBIT_PERIOD_MS) * 360) % 360;
}

export function formatDuration(durationMs: number | null): string {
  if (durationMs === null) {
    return "LIVE";
  }

  const totalSeconds = Math.max(0, Math.floor(durationMs / 1_000));
  const hours = Math.floor(totalSeconds / 3_600);
  const minutes = Math.floor((totalSeconds % 3_600) / 60);
  const seconds = totalSeconds % 60;

  return hours > 0
    ? `${hours}:${minutes.toString().padStart(2, "0")}:${seconds
        .toString()
        .padStart(2, "0")}`
    : `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

export function progressPercent(positionMs: number, durationMs: number | null): number {
  if (durationMs === null || durationMs <= 0) {
    return 0;
  }
  return Math.min(100, Math.max(0, (positionMs / durationMs) * 100));
}
