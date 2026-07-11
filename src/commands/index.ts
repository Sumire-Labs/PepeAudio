import { Collection } from 'discord.js';
import type { BotCommand } from './types.js';
import { playCommand } from './play.command.js';
import { nowCommand } from './now.command.js';
import { skipCommand } from './skip.command.js';
import { stopCommand } from './stop.command.js';
import { quitCommand } from './quit.command.js';
import { stayCommand } from './stay.command.js';

export const commands = new Collection<string, BotCommand>();
for (const cmd of [playCommand, nowCommand, skipCommand, stopCommand, quitCommand, stayCommand]) {
  commands.set(cmd.data.name, cmd);
}
