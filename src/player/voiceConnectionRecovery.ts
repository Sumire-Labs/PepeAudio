import { entersState, VoiceConnectionStatus, type VoiceConnection } from '@discordjs/voice';
import type { childLogger } from '../logger.js';

/**
 * `onLost` is a plain callback (GuildPlayer passes its own `stop()`), not a
 * back-reference, so teardown can't re-enter the mutex.
 */
export function attachConnectionRecovery(
  connection: VoiceConnection,
  log: ReturnType<typeof childLogger>,
  onLost: () => Promise<void>,
): void {
  connection.on(VoiceConnectionStatus.Disconnected, async () => {
    try {
      await Promise.race([
        entersState(connection, VoiceConnectionStatus.Signalling, 5_000),
        entersState(connection, VoiceConnectionStatus.Connecting, 5_000),
      ]);
    } catch {
      log.warn('Voice connection dropped and did not recover — tearing down');
      await onLost();
    }
  });
}
