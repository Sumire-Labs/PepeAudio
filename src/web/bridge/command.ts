/**
 * The single command executor for the web dashboard, shared by LocalBridge and
 * the shard-side IPC bridge. Runs on the owning shard (it touches
 * GuildPlayerManager + the discord.js Client). Re-authorizes every command via
 * resolveViewerCapabilities before mutating anything — the browser is trusted
 * only with the authenticated userId.
 */
import type { Client, Guild } from 'discord.js';
import * as GuildPlayerManager from '../../player/GuildPlayerManager.js';
import type { GuildPlayer } from '../../player/GuildPlayer.js';
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
import { searchYouTube } from '../../sources/youtube.js';
import type { QueueItem } from '../../player/QueueItem.js';
import { logger } from '../../logger.js';
import { resolveViewerCapabilities } from './permission.js';
import { buildSnapshot, toQueueItemDTO } from './snapshot.js';
import type { CommandResult, ResolveResult, SearchCandidate, ViewerCapabilities, WebCommand } from './types.js';

const MAX_QUERY_LENGTH = 2000;
/** How many URLs loadPlaylist resolves synchronously (to get playback started) before backgrounding the rest. */
const PLAYLIST_SYNC_CAP = 3;
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
function snapshotOrNull(guildId: string, viewer: ViewerCapabilities, guild: Guild | undefined): CommandResult {
  const player = GuildPlayerManager.get(guildId);
  return { ok: true, snapshot: player && !player.destroyed ? buildSnapshot(player, viewer, guild) : null };
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
  if (command.type === 'addTrack') return runAddTrack(guildId, userId, command.query, guild);
  if (command.type === 'loadPlaylist') return runLoadPlaylist(guildId, userId, command.sourceUrls, guild);

  const player = GuildPlayerManager.get(guildId);
  if (!player || player.destroyed) return fail('このサーバーで再生中のセッションがありません。');

  const viewer = await resolveViewerCapabilities(guild, userId, player);
  if (!viewer.canControl) return fail(viewer.denyReason ?? '権限がありません。');

  // Per-action cooldown, independent per command type (mirrors the Discord panel).
  const isVolumeLike = command.type === 'setVolume' || command.type === 'setAuraPreset' || command.type === 'seek';
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
      case 'jumpTo':
        if (typeof command.id !== 'string') return fail('不正なリクエストです。');
        await player.jumpToQueueItem(command.id);
        break;
      case 'seek': {
        const positionMs = Number(command.positionMs);
        if (!Number.isFinite(positionMs)) return fail('不正なシーク位置です。');
        await player.seek(positionMs);
        break;
      }
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

  return snapshotOrNull(guildId, viewer, guild);
}

/**
 * Gets (creating if needed) the guild's player and authorizes the caller. New
 * sessions require the caller to be in a voice channel (so we can read the VC +
 * adapterCreator), matching acquireGuildPlayer.
 */
async function ensurePlayerForAdd(
  guildId: string,
  userId: string,
  guild: Guild,
): Promise<{ player: GuildPlayer; viewer: ViewerCapabilities } | { error: string }> {
  let player = GuildPlayerManager.get(guildId);
  if (!player || player.destroyed) {
    const member = guild.members.cache.get(userId) ?? (await guild.members.fetch(userId).catch(() => null));
    const voiceChannel = member?.voice.channel;
    if (!voiceChannel) return { error: '新しく再生を始めるにはボイスチャンネルに参加してください。' };

    player = GuildPlayerManager.getOrCreate({
      guildId,
      // The web UI is the panel, so there's no natural text channel — use the
      // voice channel's own chat. No panel is auto-sent for web-initiated sessions.
      textChannelId: voiceChannel.id,
      voiceChannelId: voiceChannel.id,
      adapterCreator: guild.voiceAdapterCreator,
      ffmpeg: getFfmpegCapabilities(),
    });
    try {
      await player.waitUntilReady();
    } catch (err) {
      logger.error({ err, guildId }, 'Web add: voice connection failed to become ready');
      await GuildPlayerManager.destroy(guildId);
      return { error: 'ボイスチャンネルへの接続に失敗しました。もう一度お試しください。' };
    }
  }

  const viewer = await resolveViewerCapabilities(guild, userId, player);
  if (!viewer.canControl) return { error: viewer.denyReason ?? '権限がありません。' };
  return { player, viewer };
}

/** Enqueues resolved items and starts playback if the player was idle. */
async function enqueueAndMaybeStart(player: GuildPlayer, items: QueueItem[]): Promise<number> {
  if (items.length === 0) return 0;
  const wasIdle = !player.currentTrack;
  const added = player.enqueue(items);
  if (added > 0 && wasIdle && !player.destroyed) await player.playNext();
  return added;
}

async function runAddTrack(guildId: string, userId: string, query: unknown, guild: Guild): Promise<CommandResult> {
  if (!checkCooldown('play', userId, PLAY_COOLDOWN_MS)) return fail('少し間隔を空けてください。');
  if (typeof query !== 'string' || !query.trim()) return fail('URL または検索語を入力してください。');
  if (query.length > MAX_QUERY_LENGTH) return fail('入力が長すぎます。');

  const ensured = await ensurePlayerForAdd(guildId, userId, guild);
  if ('error' in ensured) return fail(ensured.error);
  const { player, viewer } = ensured;

  let items: QueueItem[];
  try {
    items = await resolveInput(query.trim(), userId); // SSRF-guarded via classifyInput
  } catch (err) {
    return fail(mapResolveError(err, query));
  }
  if (items.length === 0) return fail('再生できる曲が見つかりませんでした。');

  const added = await enqueueAndMaybeStart(player, items);
  if (added === 0) return fail('キューが上限に達しているため追加できませんでした。');
  return snapshotOrNull(guildId, viewer, guild);
}

/**
 * Loads a saved playlist into the queue WITHOUT blocking the request on every
 * track's network resolve. It resolves just enough (up to PLAYLIST_SYNC_CAP) to
 * get playback going, returns immediately, then resolves + enqueues the rest in
 * the background — each enqueue emits an update that streams to the browser over
 * SSE. This keeps a big playlist from hanging the HTTP request (and hitting
 * reverse-proxy timeouts).
 */
async function runLoadPlaylist(guildId: string, userId: string, sourceUrls: unknown, guild: Guild): Promise<CommandResult> {
  if (!checkCooldown('play', userId, PLAY_COOLDOWN_MS)) return fail('少し間隔を空けてください。');
  const urls = (Array.isArray(sourceUrls) ? sourceUrls : [])
    .filter((u): u is string => typeof u === 'string' && u.trim().length > 0 && u.length <= MAX_QUERY_LENGTH)
    .slice(0, MAX_PLAYLIST_TRACKS);
  if (urls.length === 0) return fail('プレイリストに曲がありません。');

  const ensured = await ensurePlayerForAdd(guildId, userId, guild);
  if ('error' in ensured) return fail(ensured.error);
  const { player, viewer } = ensured;

  // Resolve synchronously until we have at least one playable track (bounded).
  const initial: QueueItem[] = [];
  let cursor = 0;
  for (; cursor < urls.length && cursor < PLAYLIST_SYNC_CAP && initial.length === 0; cursor++) {
    try {
      initial.push(...(await resolveInput(urls[cursor]!.trim(), userId)));
    } catch (err) {
      logger.warn({ err, url: urls[cursor] }, 'Web loadPlaylist: skipping an unresolvable track');
    }
  }
  if (initial.length === 0 && cursor >= urls.length) return fail('再生できる曲が見つかりませんでした。');

  await enqueueAndMaybeStart(player, initial);

  const remaining = urls.slice(cursor);
  if (remaining.length > 0) {
    void resolveRestInBackground(guildId, userId, remaining).catch((err) =>
      logger.error({ err, guildId }, 'Web loadPlaylist: background resolve failed'),
    );
  }
  return snapshotOrNull(guildId, viewer, guild);
}

/** Resolves + enqueues the remaining playlist URLs one at a time, after the response is sent. */
async function resolveRestInBackground(guildId: string, userId: string, urls: string[]): Promise<void> {
  for (const url of urls) {
    const player = GuildPlayerManager.get(guildId);
    if (!player || player.destroyed) return; // session ended — stop working
    let items: QueueItem[];
    try {
      items = await resolveInput(url.trim(), userId);
    } catch (err) {
      logger.warn({ err, url }, 'Web loadPlaylist(bg): skipping an unresolvable track');
      continue;
    }
    await enqueueAndMaybeStart(player, items);
  }
}

/**
 * Runs a YouTube search and returns lightweight candidates (no enqueue). Used by
 * the dashboard's "pick from search results" flow. Guild-independent, so the
 * sharded bridge can run it on any shard.
 */
export async function runWebSearch(query: string): Promise<SearchCandidate[]> {
  const results = await searchYouTube(query, 6);
  return results.map((r) => ({
    title: r.title,
    author: r.author,
    url: r.url,
    thumbnailUrl: `https://i.ytimg.com/vi/${r.videoId}/mqdefault.jpg`,
  }));
}

/**
 * Resolves a URL/search to track DTOs WITHOUT enqueuing — for importing a
 * playlist into a SAVED playlist. Reuses resolveInput (so playlist URLs expand
 * and SSRF guards apply). Guild-independent, so the sharded bridge can run it on
 * any shard. Lazy items (Spotify/Apple playlists) come back with null
 * duration/thumbnail; the saved playlist stores them by title+artist (see
 * playlistRepo), which the loader resolves via search.
 */
export async function runWebResolve(query: string): Promise<ResolveResult> {
  if (typeof query !== 'string' || !query.trim()) return { tracks: [], error: 'URL を入力してください。' };
  try {
    const items = await resolveInput(query.trim(), 'web-import');
    return { tracks: items.slice(0, MAX_PLAYLIST_TRACKS).map((item) => toQueueItemDTO(item)) };
  } catch (err) {
    return { tracks: [], error: mapResolveError(err, query) };
  }
}
