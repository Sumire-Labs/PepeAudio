import { Events, MessageFlags, type Client, type Interaction } from 'discord.js';
import { commands } from '../commands/index.js';
import { handleButtonOrSelect } from './panelActionHandler.js';
import { handleAddQueueModalSubmit } from './addQueueModalHandler.js';
import { isDeadInteractionError, safeReply } from '../util/interactionSafety.js';
import { logger } from '../logger.js';

export function registerInteractionCreateEvent(client: Client): void {
  client.on(Events.InteractionCreate, async (interaction: Interaction) => {
    try {
      if (interaction.isChatInputCommand()) {
        const command = commands.get(interaction.commandName);
        if (!command) return;
        await command.execute(interaction);
        return;
      }
      if (interaction.isButton() || interaction.isStringSelectMenu()) {
        await handleButtonOrSelect(interaction);
        return;
      }
      if (interaction.isModalSubmit()) {
        await handleAddQueueModalSubmit(interaction);
        return;
      }
    } catch (err) {
      // Expected when a user clicks a stale panel or the 3s response window
      // closed before we could answer — there's nothing to recover and trying
      // to reply would just fail again, so log quietly and stop.
      if (isDeadInteractionError(err)) {
        logger.debug({ err }, 'Interaction expired or was already handled before we could respond');
        return;
      }
      logger.error({ err }, 'Unhandled interaction error');
      if (interaction.isRepliable()) {
        await safeReply(interaction, { content: '予期しないエラーが発生しました。', flags: MessageFlags.Ephemeral }).catch(
          (nestedErr) => logger.debug({ err: nestedErr }, 'Failed to send error reply'),
        );
      }
    }
  });
}
