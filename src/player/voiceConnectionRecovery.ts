import { entersState, VoiceConnectionStatus, type VoiceConnection } from '@discordjs/voice';
import type { childLogger } from '../logger.js';

/**
 * Wires up automatic teardown when the voice connection drops and doesn't
 * recover on its own within a short grace window. Never touches enqueueAction
 * or holds a GuildPlayer back-reference - `onLost` is a plain callback
 * (GuildPlayer passes its own `stop()`), so this can't re-enter the mutex.
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
