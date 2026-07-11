import { existsSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import ffmpegStatic from 'ffmpeg-static';
import { logger } from '../logger.js';
import { env } from './env.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(__dirname, '..', '..');
const LOCAL_BIN = path.join(PROJECT_ROOT, 'bin', process.platform === 'win32' ? 'ffmpeg.exe' : 'ffmpeg');

/**
 * Resolution order: explicit FFMPEG_PATH override > a locally staged binary
 * (populated by `npm run setup-ffmpeg`, potentially sofalizer-capable) >
 * the guaranteed-available ffmpeg-static binary (no sofalizer support).
 */
export function resolveFfmpegPath(): string {
  if (env.ffmpegPathOverride && existsSync(env.ffmpegPathOverride)) {
    return env.ffmpegPathOverride;
  }
  if (existsSync(LOCAL_BIN)) {
    return LOCAL_BIN;
  }
  if (!ffmpegStatic) {
    throw new Error(
      'No ffmpeg binary available: set FFMPEG_PATH, run `npm run setup-ffmpeg`, or check the ffmpeg-static install.',
    );
  }
  return ffmpegStatic;
}

export function probeSofalizerSupport(ffmpegPath: string): boolean {
  try {
    const output = execFileSync(ffmpegPath, ['-hide_banner', '-filters'], { encoding: 'utf8' });
    return output.includes('sofalizer');
  } catch (err) {
    logger.warn({ err, ffmpegPath }, 'Failed to probe ffmpeg filter list');
    return false;
  }
}

export interface FfmpegCapabilities {
  path: string;
  sofalizerAvailable: boolean;
}

/**
 * Resolves the ffmpeg binary, wires FFMPEG_PATH for prism-media to pick up,
 * and probes whether the true binaural (sofalizer/libmysofa) filter is usable
 * on this host. Call once at startup; the result is cached by the caller.
 */
export function initFfmpeg(): FfmpegCapabilities {
  const resolvedPath = resolveFfmpegPath();
  process.env.FFMPEG_PATH = resolvedPath;
  const sofalizerAvailable = probeSofalizerSupport(resolvedPath);
  logger.info(
    { ffmpegPath: resolvedPath, sofalizerAvailable },
    sofalizerAvailable
      ? '3D audio: sofalizer (binaural HRTF) is available'
      : '3D audio: sofalizer unavailable on this ffmpeg build - falling back to the lightweight spatial filter chain',
  );
  return { path: resolvedPath, sofalizerAvailable };
}
