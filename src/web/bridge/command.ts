/**
 * The single command executor for the web dashboard, shared by LocalBridge and
 * the shard-side IPC bridge. Runs ON THE OWNING SHARD (it touches
 * GuildPlayerManager + the discord.js Client). Re-authorizes EVERY command via
 * resolveViewerCapabilities before mutating anything — the browser is trusted
 * for nothing beyond the authenticated userId.
 */
import type { Client, Guild } from 'discord.js';
import * as GuildPlayerManager from '../../player/GuildPlayerManager.js';
import { getFfmpegCapabilities } from '../../config/ffmpegState.js';
import { checkCooldown } from '../../util/rateLimiter.js';
import {
  AURA_ENABLED,
  BUTTON_COOLDOWN_MS,
  MAX_PLAYLIST_TRACKS,
  PLAY_COOLDOWN_MS,
  VOLUME_COOLDOWN_MS,
} from '../../player/constants.js';
import {
  resolveInput,
  SourceResolutionError,
  YouTubeUnavailableError,
  NoMatchFoundError,
  SpotifyResolutionError,
  SoundCloudUnavailableError,
  AppleMusicResolutionError,
} from '../../sources/index.js';
import type { QueueItem } from '../../player/QueueItem.js';
import { logger } from '../../logger.js';
import { resolveViewerCapabilities } from './permission.js';
import { buildSnapshot } from './snapshot.js';
import type { CommandResult, WebCommand } from './types.js';

const MAX_QUERY_LENGTH = 2000;
const LOOP_MODES = new Set(['off', 'track', 'queue']);
const TOGGLE_VALUES = new Set(['on', 'off']);

const KNOWN_RESOLVE_ERRORS = [
  SourceResolutionError,
  YouTubeUnavailableError,
  NoMatchFoundError,
  SpotifyResolutionError,
  SoundCloudUnavailableError,
  AppleMusicResolutionError,
];

/** Turns a resolver error into a user-safe message, mirroring resolvePlayQuery. */
function mapResolveError(err: unknown, query: string): string {
  if (KNOWN_RESOLVE_ERRORS.some((ErrorType) => err instanceof ErrorType)) {
    return (err as Error).message;
  }
  logger.error({ err, query }, 'Web addTrack: unhandled resolver error');
  return '再生できませんでした。リンクを確認してください。';
}

function fail(error: string): CommandResult {
  return { ok: false, error };
}

/** Snapshot of the (possibly just-mutated) player, or null if the session ended. */
function snapshotOrNull(guildId: string, viewer: Awaited<ReturnType<typeof resolveViewerCapabilities>>): CommandResult {
  const player = GuildPlayerManager.get(guildId);
  return { ok: true, snapshot: player && !player.destroyed ? buildSnapshot(player, viewer) : null };
}

export async function runWebCommand(
  guildId: string,
  userId: string,
  command: WebCommand,
  client: Client,
): Promise<CommandResult> {
  const guild = client.guilds.cache.get(guildId);
  if (!guild) return fail('サーバーが見つかりません。');

  // addTrack / loadPlaylist may need to CREATE a session; every other command
  // requires an existing player.
  if (command.type === 'addTrack' || command.type === 'loadPlaylist') {
    return runAddCommand(guildId, userId, command, guild, client);
  }

  const player = GuildPlayerManager.get(guildId);
  if (!player || player.destroyed) return fail('このサーバーで再生中のセッションがありません。');

  const viewer = await resolveViewerCapabilities(guild, userId, player);
  if (!viewer.canControl) return fail(viewer.denyReason ?? '権限がありません。');

  // Per-action cooldown, independent per command type (mirrors the Discord panel).
  const isVolumeLike = command.type === 'setVolume' || command.type === 'setAuraPreset';
  const cooldownMs = isVolumeLike ? VOLUME_COOLDOWN_MS : BUTTON_COOLDOWN_MS;
  if (!checkCooldown(`web:${command.type}`, userId, cooldownMs)) {
    return fail('少し間隔を空けてください。');
  }

  try {
    switch (command.type) {
      case 'skip':
        await player.skip();
        break;
      case 'previous': {
        const result = await player.previous();
        if (!result.ok) {
          return fail(result.reason === 'no-history' ? '前の曲はありません。' : '前の曲の再生に失敗しました。');
        }
        break;
      }
      case 'togglePlayPause':
        if (player.isPaused()) await player.resume();
        else await player.pause();
        break;
      case 'pause':
        await player.pause();
        break;
      case 'resume':
        await player.resume();
        break;
      case 'stop':
        await player.stop();
        break;
      case 'toggleShuffle':
        player.toggleShuffle();
        break;
      case 'setAutoplay':
        player.setAutoplay(Boolean(command.enabled));
        break;
      case 'setStay247':
        player.setStay247(Boolean(command.enabled));
        break;
      case 'setLoopMode':
        if (!LOOP_MODES.has(command.mode)) return fail('不正なループ設定です。');
        player.setLoopMode(command.mode);
        break;
      case 'setVolume': {
        const percent = Number(command.percent);
        if (!Number.isFinite(percent)) return fail('不正な音量値です。');
        player.setVolume(percent); // GuildPlayer clamps to 0..MAX_VOLUME_PERCENT and rounds
        break;
      }
      case 'setHrir':
        if (!TOGGLE_VALUES.has(command.mode)) return fail('不正な値です。');
        if (AURA_ENABLED) await player.setHrirMode(command.mode);
        break;
      case 'setAura360':
        if (!TOGGLE_VALUES.has(command.mode)) return fail('不正な値です。');
        if (AURA_ENABLED) await player.setAura360Mode(command.mode);
        break;
      case 'setAuraPreset':
        if (AURA_ENABLED && typeof command.id === 'string') await player.setAuraPreset(command.id);
        break;
      case 'removeQueueItem':
        if (typeof command.id !== 'string') return fail('不正なリクエストです。');
        await player.removeQueueItem(command.id);
        break;
      case 'moveQueueItem': {
        const toIndex = Number(command.toIndex);
        if (typeof command.id !== 'string' || !Number.isFinite(toIndex)) return fail('不正なリクエストです。');
        await player.moveQueueItem(command.id, Math.floor(toIndex));
        break;
      }
      case 'clearQueue':
        await player.clearQueue();
        break;
      default: {
        // Exhaustiveness guard: addTrack/loadPlaylist handled above.
        const _never: never = command;
        return fail('未対応の操作です。');
      }
    }
  } catch (err) {
    logger.error({ err, command: command.type, guildId }, 'Web command handler failed');
    return fail('操作に失敗しました。もう一度お試しください。');
  }

  return snapshotOrNull(guildId, viewer);
}

/**
 * Handles addTrack (single query) and loadPlaylist (many source URLs), including
 * creating the session when the bot isn't connected yet. Requires the requester
 * to be in a voice channel to start a NEW session (so we can read the VC +
 * adapterCreator), matching acquireGuildPlayer.
 */
async function runAddCommand(
  guildId: string,
  userId: string,
  command: Extract<WebCommand, { type: 'addTrack' | 'loadPlaylist' }>,
  guild: Guild,
  _client: Client,
): Promise<CommandResult> {
  // Share the /play anti-abuse bucket so a user can't bypass it via the web.
  if (!checkCooldown('play', userId, PLAY_COOLDOWN_MS)) {
    return fail('少し間隔を空けてください。');
  }

  const queries: string[] =
    command.type === 'addTrack'
      ? [command.query]
      : command.sourceUrls.slice(0, MAX_PLAYLIST_TRACKS);

  if (command.type === 'addTrack') {
    if (typeof command.query !== 'string' || !command.query.trim()) return fail('URL または検索語を入力してください。');
    if (command.query.length > MAX_QUERY_LENGTH) return fail('入力が長すぎます。');
  } else if (queries.length === 0) {
    return fail('プレイリストに曲がありません。');
  }

  let player = GuildPlayerManager.get(guildId);

  if (!player || player.destroyed) {
    const member = guild.members.cache.get(userId) ?? (await guild.members.fetch(userId).catch(() => null));
    const voiceChannel = member?.voice.channel;
    if (!voiceChannel) return fail('新しく再生を始めるにはボイスチャンネルに参加してください。');

    player = GuildPlayerManager.getOrCreate({
      guildId,
      // The web UI is the panel, so there's no natural text channel — use the
      // voice channel's own chat (voice channels accept messages). No panel is
      // auto-sent for web-initiated sessions.
      textChannelId: voiceChannel.id,
      voiceChannelId: voiceChannel.id,
      adapterCreator: guild.voiceAdapterCreator,
      ffmpeg: getFfmpegCapabilities(),
    });
    try {
      await player.waitUntilReady();
    } catch (err) {
      logger.error({ err, guildId }, 'Web addTrack: voice connection failed to become ready');
      await GuildPlayerManager.destroy(guildId);
      return fail('ボイスチャンネルへの接続に失敗しました。もう一度お試しください。');
    }
  }

  // Authorize only after we know the player/VC (the same-VC rule needs player.voiceChannelId).
  const viewer = await resolveViewerCapabilities(guild, userId, player);
  if (!viewer.canControl) return fail(viewer.denyReason ?? '権限がありません。');

  // Resolve every query (SSRF-guarded via resolveInput's classifyInput). A single
  // failed query in a playlist is skipped; a failed single addTrack is reported.
  const items: QueueItem[] = [];
  for (const query of queries) {
    if (typeof query !== 'string' || !query.trim() || query.length > MAX_QUERY_LENGTH) continue;
    try {
      const resolved = await resolveInput(query.trim(), userId);
      items.push(...resolved);
    } catch (err) {
      if (command.type === 'addTrack') return fail(mapResolveError(err, query));
      logger.warn({ err, query }, 'Web loadPlaylist: skipping a track that failed to resolve');
    }
    if (items.length >= MAX_PLAYLIST_TRACKS) break;
  }
  if (items.length === 0) return fail('再生できる曲が見つかりませんでした。');

  const wasIdle = !player.currentTrack;
  const addedCount = player.enqueue(items);
  if (addedCount === 0) return fail('キューが上限に達しているため追加できませんでした。');
  if (wasIdle) await player.playNext(); // enqueue() alone never starts playback (see enqueueAndConfirm.ts)

  return snapshotOrNull(guildId, viewer);
}
