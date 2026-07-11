import 'dotenv/config';
import { REST, Routes } from 'discord.js';
import { commands } from '../src/commands/index.js';

const token = process.env.DISCORD_TOKEN;
const clientId = process.env.CLIENT_ID;
const guildId = process.env.GUILD_ID;

if (!token || !clientId) {
  throw new Error('DISCORD_TOKEN and CLIENT_ID must be set (see .env.example)');
}

const body = commands.map((cmd) => cmd.data.toJSON());
const rest = new REST().setToken(token);

const route = guildId ? Routes.applicationGuildCommands(clientId, guildId) : Routes.applicationCommands(clientId);

console.log(`Registering ${body.length} command(s) ${guildId ? `to guild ${guildId} (instant)` : 'globally (may take up to 1h to propagate)'}...`);
await rest.put(route, { body });
console.log('Done.');
