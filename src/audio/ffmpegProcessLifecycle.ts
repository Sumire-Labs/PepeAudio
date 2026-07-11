import type { Readable } from 'node:stream';
import { spawn, type ChildProcess } from 'node:child_process';
import { createAudioResource, StreamType } from '@discordjs/voice';
import { logger } from '../logger.js';
import { acquireSofalizerSlot, releaseSofalizerSlot, acquireHrirSlot, releaseHrirSlot } from './ffmpegConcurrencySlots.js';
import type { FfmpegSpawnPlan } from './ffmpegPlan.js';
import type { CreateTrackResourceParams, TrackResource } from './resourceTypes.js';

/**
 * Spawns the ffmpeg-backed resource (spatial/HRIR/seek path) per a decided FfmpegSpawnPlan,
 * wires up concurrency-slot bookkeeping, diagnostics, and stream piping, and builds the
 * final AudioResource with inline volume always on (invariant #4's non-fast-path branch —
 * volume is never an ffmpeg filter, only ever the inline VolumeTransformer).
 */
export function spawnFfmpegResource(plan: FfmpegSpawnPlan, params: CreateTrackResourceParams): TrackResource {
  const { stream, ffmpegPath, volumePercent } = params;
  const { useHrir, useSofalizer, args } = plan;

  // `resourceFactory.ts` spawns ffmpeg directly via node:child_process's spawn(),
  // NEVER prism-media's `FFmpeg` class: that class resolves its own binary via
  // `require('ffmpeg-static')` first (ignoring any env var) and caches the
  // result for the process's lifetime — confirmed by hitting "No such filter:
  // 'sofalizer'" at runtime even though our own resolved binary supports it.
  // Spawning directly with `ffmpegPath` is the only way to guarantee the binary
  // we actually probed is the one that runs.
  const ffmpegProcess = spawn(ffmpegPath, args, { windowsHide: true });

  if (useSofalizer) {
    acquireSofalizerSlot(ffmpegProcess);
    ffmpegProcess.once('exit', () => releaseSofalizerSlot(ffmpegProcess));
    ffmpegProcess.once('error', () => releaseSofalizerSlot(ffmpegProcess));
  }
  if (useHrir) {
    acquireHrirSlot(ffmpegProcess);
    ffmpegProcess.once('exit', () => releaseHrirSlot(ffmpegProcess));
    ffmpegProcess.once('error', () => releaseHrirSlot(ffmpegProcess));
  }

  // Diagnostics: `-loglevel error` means anything ffmpeg writes to stderr is a
  // real problem (bad input, filter failure, etc.) — surface it, since a
  // silently-dying ffmpeg process looks identical to "track ended" otherwise.
  ffmpegProcess.stderr?.on('data', (chunk: Buffer) => {
    logger.error({ ffmpegPath, ffmpegArgs: args, stderr: chunk.toString('utf8').trim() }, 'ffmpeg (audio pipeline) reported an error');
  });
  ffmpegProcess.once('exit', (code, signal) => {
    // code 0 / signal SIGKILL (our own teardown) are expected; anything else is worth seeing by default.
    const level = code === 0 || signal === 'SIGKILL' ? 'debug' : 'warn';
    logger[level]({ code, signal }, 'ffmpeg (audio pipeline) process exited');
  });
  ffmpegProcess.once('error', (err) => {
    logger.error({ err, ffmpegPath }, 'Failed to spawn the ffmpeg audio pipeline process');
  });

  // A killed process can make the still-piping source stream see EPIPE/ECONNRESET;
  // swallow it here since destroyFfmpegProcess() always unpipes first, and the
  // crash-worthy error already gets logged there.
  stream.on('error', (err) => {
    logger.debug({ err }, 'Source stream error (expected if ffmpeg was just torn down)');
  });
  if (ffmpegProcess.stdin) {
    stream.pipe(ffmpegProcess.stdin);
    ffmpegProcess.stdin.on('error', (err) => {
      logger.debug({ err }, 'ffmpeg stdin error (expected if the process was just torn down)');
    });
  }

  const resource = createAudioResource(ffmpegProcess.stdout ?? stream, {
    inputType: StreamType.Raw,
    inlineVolume: true,
  });
  resource.volume?.setVolumeLogarithmic(volumePercent / 100);

  return { resource, ffmpegProcess, usingSofalizer: useSofalizer, usingHrir: useHrir, hasInlineVolume: true };
}

/**
 * Correct teardown for a directly-spawned ffmpeg process mid-stream: unpipe the
 * source first (avoids an EPIPE crash), then SIGKILL the process.
 */
export function destroyFfmpegProcess(ffmpegProcess: ChildProcess, sourceStream?: Readable): void {
  if (sourceStream && ffmpegProcess.stdin) {
    sourceStream.unpipe(ffmpegProcess.stdin);
  }
  ffmpegProcess.kill('SIGKILL');
  // Release synchronously rather than relying solely on 'exit'/'error' firing —
  // confirmed in testing that the concurrency counter could get stuck otherwise.
  // Both are safe to call unconditionally: each is a no-op if this process
  // never held that particular slot.
  releaseSofalizerSlot(ffmpegProcess);
  releaseHrirSlot(ffmpegProcess);
}
