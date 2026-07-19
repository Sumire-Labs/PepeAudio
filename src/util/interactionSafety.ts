import {
  DiscordAPIError,
  type RepliableInteraction,
  type MessageComponentInteraction,
  type InteractionReplyOptions,
} from 'discord.js';
import { logger } from '../logger.js';

// Codes for an interaction that can no longer be answered (stale button click,
// expired 3s window). Swallowed at debug to avoid the 10062 -> 40060 double-log flood.
const DEAD_INTERACTION_CODES = new Set<number>([
  10062, // Unknown interaction — token expired (>3s) or already consumed
  40060, // Interaction has already been acknowledged
  10008, // Unknown message — the original message is gone
]);

export function isDeadInteractionError(err: unknown): boolean {
  return err instanceof DiscordAPIError && DEAD_INTERACTION_CODES.has(Number(err.code));
}

/** Auto-picks followUp vs reply; swallows dead-interaction errors, re-throws real ones. */
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

/** Returns false if the interaction was already gone; callers should bail. */
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
