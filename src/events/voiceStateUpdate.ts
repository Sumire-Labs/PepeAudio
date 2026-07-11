import type { Client } from 'discord.js';
import * as GuildPlayerManager from '../player/GuildPlayerManager.js';
import { logger } from '../logger.js';

/** Tracks "everyone left the voice channel" to start/cancel the alone-timeout on GuildPlayer. */
export function registerVoiceStateUpdateEvent(client: Client): void {
  client.on('voiceStateUpdate', (oldState) => {
    const guildId = oldState.guild.id;
    const player = GuildPlayerManager.get(guildId);
    if (!player || player.destroyed) return;

    const channel = oldState.guild.channels.cache.get(player.voiceChannelId);
    if (!channel || !channel.isVoiceBased()) return;

    const nonBotMembers = channel.members.filter((m) => !m.user.bot).size;
    if (nonBotMembers === 0) {
      player.startAloneTimer(() => {
        logger.info({ guildId }, 'Alone in voice channel timeout reached - disconnecting');
        void player.stop();
      });
    } else {
      player.cancelAloneTimer();
    }
  });
}
