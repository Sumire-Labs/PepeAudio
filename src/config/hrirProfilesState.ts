import { loadHrirProfiles, type HrirProfile } from './hrirProfiles.js';
import { logger } from '../logger.js';

let profiles: HrirProfile[] = [];

/** Call once at startup, after initFfmpeg() (channel-count probing needs the resolved ffmpeg binary). */
export function initHrirProfiles(ffmpegPath: string, dirOverride?: string | null): HrirProfile[] {
  profiles = dirOverride ? loadHrirProfiles(ffmpegPath, dirOverride) : loadHrirProfiles(ffmpegPath);
  logger.info(
    { count: profiles.length, profiles: profiles.map((p) => ({ id: p.id, format: p.format })) },
    profiles.length > 0
      ? 'HRIR profiles loaded'
      : 'No HRIR profiles found (folder empty or missing - the feature stays unavailable)',
  );
  return profiles;
}

export function getHrirProfiles(): HrirProfile[] {
  return profiles;
}

export function getHrirProfileById(id: string): HrirProfile | undefined {
  return profiles.find((p) => p.id === id);
}
