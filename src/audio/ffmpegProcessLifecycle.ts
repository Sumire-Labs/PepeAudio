import type { Readable } from 'node:stream';
import { spawn, type ChildProcess } from 'node:child_process';
import { createAudioResource, StreamType } from '@discordjs/voice';
import { logger } from '../logger.js';
import { acquireSofalizerSlot, releaseSofalizerSlot, acquireHrirSlot, releaseHrirSlot } from './ffmpegConcurrencySlots.js';
import type { FfmpegSpawnPlan } from './ffmpegPlan.js';
import type { CreateTrackResourceParams, TrackResource } from './resourceTypes.js';

/** Volume is never an ffmpeg filter here — only ever the inline VolumeTransformer. */
export function spawnFfmpegResource(plan: FfmpegSpawnPlan, params: CreateTrackResourceParams, trimFactor = 1): TrackResource {
  const { stream, ffmpegPath, volumePercent } = params;
  const { useHrir, useSofalizer, args } = plan;

  // Spawn directly, NEVER prism-media's FFmpeg class: it resolves its binary via
  // require('ffmpeg-static') (ignoring env) and caches it process-wide, which can run a
  // binary lacking sofalizer. Direct spawn guarantees the binary we probed is the one that runs.
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

  // With -loglevel error, anything on stderr is a real problem — surface it, since a
  // silently-dying ffmpeg looks identical to "track ended".
  ffmpegProcess.stderr?.on('data', (chunk: Buffer) => {
    logger.error({ ffmpegPath, ffmpegArgs: args, stderr: chunk.toString('utf8').trim() }, 'ffmpeg (audio pipeline) reported an error');
  });
  ffmpegProcess.once('exit', (code, signal) => {
    // code 0 / SIGKILL (our own teardown) are expected; anything else is worth seeing.
    const level = code === 0 || signal === 'SIGKILL' ? 'debug' : 'warn';
    logger[level]({ code, signal }, 'ffmpeg (audio pipeline) process exited');
  });
  ffmpegProcess.once('error', (err) => {
    logger.error({ err, ffmpegPath }, 'Failed to spawn the ffmpeg audio pipeline process');
  });

  // A killed process can make the still-piping source see EPIPE/ECONNRESET; swallow it
  // (destroyFfmpegProcess unpipes first, and real errors are logged there). No stream on the
  // buffered input-seek path — ffmpeg reads the temp file itself, so there's nothing to pipe.
  stream?.on('error', (err) => {
    logger.debug({ err }, 'Source stream error (expected if ffmpeg was just torn down)');
  });
  if (stream && ffmpegProcess.stdin) {
    stream.pipe(ffmpegProcess.stdin);
    ffmpegProcess.stdin.on('error', (err) => {
      logger.debug({ err }, 'ffmpeg stdin error (expected if the process was just torn down)');
    });
  }

  const resourceInput = ffmpegProcess.stdout ?? stream;
  if (!resourceInput) {
    throw new Error('ffmpeg produced no stdout to read from');
  }
  const resource = createAudioResource(resourceInput, {
    inputType: StreamType.Raw,
    inlineVolume: true,
  });
  resource.volume?.setVolumeLogarithmic(volumePercent / 100);
  // Normal mode (Aura 360° off + seek): trim to match the quieter 360°-on level; unity otherwise.
  if (resource.volume && trimFactor !== 1) {
    resource.volume.setVolume(resource.volume.volume * trimFactor);
  }
  // Raw PCM re-encode: @discordjs/voice defaults libopus to OPUS_AUTO (~96 kbps); bump to
  // 128 kbps for headroom against re-encode grit (capped by the channel bitrate anyway).
  resource.encoder?.setBitrate(128_000);

  return { resource, ffmpegProcess, usingSofalizer: useSofalizer, usingHrir: useHrir, hasInlineVolume: true };
}

/** Unpipe the source before SIGKILL — killing first causes an EPIPE crash. */
export function destroyFfmpegProcess(ffmpegProcess: ChildProcess, sourceStream?: Readable): void {
  if (sourceStream && ffmpegProcess.stdin) {
    sourceStream.unpipe(ffmpegProcess.stdin);
  }
  ffmpegProcess.kill('SIGKILL');
  // Release synchronously — relying on 'exit'/'error' alone could leave the counter stuck.
  // Both are no-ops if this process never held that slot.
  releaseSofalizerSlot(ffmpegProcess);
  releaseHrirSlot(ffmpegProcess);
}
