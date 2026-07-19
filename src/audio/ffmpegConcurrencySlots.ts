import type { ChildProcess } from 'node:child_process';
import { MAX_HRIR_CONCURRENCY, MAX_SOFALIZER_CONCURRENCY } from '../player/constants.js';

let activeSofalizerCount = 0;
/** Set membership makes slot release idempotent across the exit/error event and explicit destroy. */
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

/** For log lines only; use hasSofalizerCapacity() for the decision. */
export function getSofalizerCount(): number {
  return activeSofalizerCount;
}

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

/** For log lines only; use hasHrirCapacity() for the decision. */
export function getHrirCount(): number {
  return activeHrirCount;
}
