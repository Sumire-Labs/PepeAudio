import { Events, MessageFlags, type Client, type Interaction } from 'discord.js';
import { commands } from '../commands/index.js';
import { handleButtonOrSelect } from './panelActionHandler.js';
import { handleAddQueueModalSubmit } from './addQueueModalHandler.js';
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
      logger.error({ err }, 'Unhandled interaction error');
      try {
        if (interaction.isRepliable()) {
          if (interaction.replied || interaction.deferred) {
            await interaction.followUp({ content: '予期しないエラーが発生しました。', flags: MessageFlags.Ephemeral });
          } else {
            await interaction.reply({ content: '予期しないエラーが発生しました。', flags: MessageFlags.Ephemeral });
          }
        }
      } catch (nestedErr) {
        logger.error({ err: nestedErr }, 'Failed to send error reply');
      }
    }
  });
}
