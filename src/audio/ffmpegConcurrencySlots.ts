import type { ChildProcess } from 'node:child_process';
import { MAX_HRIR_CONCURRENCY, MAX_SOFALIZER_CONCURRENCY } from '../player/constants.js';

let activeSofalizerCount = 0;
/** Tracks which ffmpeg processes currently hold a counted slot, so releasing is idempotent
 *  whether it happens via the 'exit'/'error' event or via an explicit destroyFfmpegProcess() call. */
const sofalizerSlotHolders = new Set<ChildProcess>();

export function acquireSofalizerSlot(ffmpegProcess: ChildProcess): void {
  activeSofalizerCount += 1;
  sofalizerSlotHolders.add(ffmpegProcess);
}

export function releaseSofalizerSlot(ffmpegProcess: ChildProcess): void {
  if (sofalizerSlotHolders.delete(ffmpegProcess)) {
    activeSofalizerCount = Math.max(0, activeSofalizerCount - 1);
  }
}

export function hasSofalizerCapacity(): boolean {
  return activeSofalizerCount < MAX_SOFALIZER_CONCURRENCY;
}

/** Exposed only for capacity-reached log lines (see ffmpegPlan.ts) - callers should prefer hasSofalizerCapacity() for the actual decision. */
export function getSofalizerCount(): number {
  return activeSofalizerCount;
}

/** Same concurrency-slot pattern as sofalizer above, kept as a separate counter/cap
 *  since the two paths are independent features that can each be enabled on their own. */
let activeHrirCount = 0;
const hrirSlotHolders = new Set<ChildProcess>();

export function acquireHrirSlot(ffmpegProcess: ChildProcess): void {
  activeHrirCount += 1;
  hrirSlotHolders.add(ffmpegProcess);
}

export function releaseHrirSlot(ffmpegProcess: ChildProcess): void {
  if (hrirSlotHolders.delete(ffmpegProcess)) {
    activeHrirCount = Math.max(0, activeHrirCount - 1);
  }
}

export function hasHrirCapacity(): boolean {
  return activeHrirCount < MAX_HRIR_CONCURRENCY;
}

/** Exposed only for capacity-reached log lines (see ffmpegPlan.ts) - callers should prefer hasHrirCapacity() for the actual decision. */
export function getHrirCount(): number {
  return activeHrirCount;
}
