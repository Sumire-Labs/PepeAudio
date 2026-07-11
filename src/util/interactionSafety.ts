import {
  DiscordAPIError,
  type RepliableInteraction,
  type MessageComponentInteraction,
  type InteractionReplyOptions,
} from 'discord.js';
import { logger } from '../logger.js';

/**
 * Discord REST error codes for an interaction that can no longer be responded
 * to. These are EXPECTED, not bugs: they fire whenever a user clicks a stale
 * panel button, double-clicks, or the gateway delivers the click so late that
 * the 3-second initial-response window has already closed. There's nothing to
 * recover, so we swallow them at debug instead of logging an error and then
 * failing a SECOND time trying to send an error reply (the 10062 -> 40060
 * double-log flood seen in the logs).
 */
const DEAD_INTERACTION_CODES = new Set<number>([
  10062, // Unknown interaction — token expired (>3s) or already consumed
  40060, // Interaction has already been acknowledged
  10008, // Unknown message — the original message is gone
]);

export function isDeadInteractionError(err: unknown): boolean {
  return err instanceof DiscordAPIError && DEAD_INTERACTION_CODES.has(Number(err.code));
}

/**
 * Replies to (or follows up on) an interaction, swallowing the dead-interaction
 * errors above. Picks followUp vs reply automatically from whether the
 * interaction has already been acknowledged, so callers don't have to track it.
 * Re-throws anything that ISN'T a dead-interaction error (a genuine failure the
 * caller may still want to see).
 */
export async function safeReply(interaction: RepliableInteraction, options: InteractionReplyOptions): Promise<void> {
  try {
    if (interaction.replied || interaction.deferred) {
      await interaction.followUp(options);
    } else {
      await interaction.reply(options);
    }
  } catch (err) {
    if (isDeadInteractionError(err)) {
      logger.debug({ err }, 'Skipped replying to an expired/already-handled interaction');
      return;
    }
    throw err;
  }
}

/**
 * Acknowledges a component interaction with deferUpdate(), returning false (and
 * swallowing the error at debug) if the interaction was already gone. Callers
 * should bail when this returns false — there's no live interaction left to act on.
 */
export async function safeDeferUpdate(interaction: MessageComponentInteraction): Promise<boolean> {
  try {
    await interaction.deferUpdate();
    return true;
  } catch (err) {
    if (isDeadInteractionError(err)) {
      logger.debug({ err }, 'Skipped deferUpdate on an expired/already-handled interaction');
      return false;
    }
    throw err;
  }
}
