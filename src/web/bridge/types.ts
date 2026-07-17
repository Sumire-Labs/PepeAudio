/**
 * The contract between the web HTTP layer and the bot. Everything here is pure
 * types with NO runtime imports, so it can be imported from any process — the
 * ShardingManager (which has no discord.js Client) included. Concrete bridges
 * (LocalBridge, ShardedBridge) implement `BotBridge`; the HTTP routes depend
 * only on this interface, never on GuildPlayer directly.
 *
 * All shapes are JSON-serializable: a snapshot must survive `structuredClone`
 * across the shard IPC boundary, so no functions, class instances, or Maps.
 */

export type SourceType = 'youtube' | 'spotify' | 'soundcloud' | 'applemusic';
export type LoopMode = 'off' | 'track' | 'queue';
export type AuraToggle = 'off' | 'on';
export type PermissionMode = 'same-voice-channel' | 'dj-role' | 'requester-only';

/** A queue/current/history entry, stripped of QueueItem's functions (getStream/prefetch) and internal offset. */
export interface QueueItemDTO {
  id: string;
  title: string;
  artist: string;
  durationMs: number | null;
  thumbnailUrl: string | null;
  sourceType: SourceType;
  sourceUrl: string;
  /** Discord user id of who requested it; the frontend resolves this to a name/avatar separately. */
  requestedBy: string;
}

/** What the current viewer (the authenticated Discord user) is allowed to do with this session. */
export interface ViewerCapabilities {
  canControl: boolean;
  /** User-safe reason (Japanese, mirrors the Discord panel strings) when canControl is false. */
  denyReason: string | null;
  inBotVoiceChannel: boolean;
}

/** Full display + control state for one guild's player, as sent to the browser (initial fetch + every SSE push). */
export interface GuildSnapshot {
  guildId: string;
  status: 'idle' | 'playing' | 'paused';
  current: QueueItemDTO | null;
  /** player.getElapsedMs() sampled at build time; combine with serverTimeMs + status to extrapolate client-side. */
  elapsedMs: number;
  queue: QueueItemDTO[];
  /** Capped (most-recent-first is NOT assumed; same order as player.history) to keep SSE frames small. */
  history: QueueItemDTO[];
  loopMode: LoopMode;
  shuffleEnabled: boolean;
  autoplay: boolean;
  volume: number;
  hrirMode: AuraToggle;
  aura360Mode: AuraToggle;
  hrirProfile: string | null;
  /** Available Aura Presets (id + label) so the frontend can render the select without a second call. */
  auraPresets: Array<{ id: string; label: string }>;
  stay247: boolean;
  permissionMode: PermissionMode;
  voiceChannelId: string;
  lastError: string | null;
  /** Whether the Aura (3D audio) feature is enabled at all in this build. */
  auraEnabled: boolean;
  viewer: ViewerCapabilities;
}

/** A YouTube search result for the "pick from search" add-track flow. */
export interface SearchCandidate {
  title: string;
  author: string;
  url: string;
  thumbnailUrl: string;
}

/**
 * One row in the guild picker: a guild the viewer shares with the bot (the bot
 * is a member, so a session can be started there even if none is active yet).
 */
export interface GuildSummary {
  guildId: string;
  name: string;
  iconUrl: string | null;
  hasActiveSession: boolean;
  status: 'idle' | 'playing' | 'paused';
  currentTitle: string | null;
}

/**
 * Every state-changing operation the browser can request. A discriminated union
 * so the command executor exhaustively handles each and rejects anything else.
 * Numeric/enum payloads are re-validated server-side before touching the player.
 */
export type WebCommand =
  | { type: 'skip' }
  | { type: 'previous' }
  | { type: 'pause' }
  | { type: 'resume' }
  | { type: 'togglePlayPause' }
  | { type: 'stop' }
  | { type: 'toggleShuffle' }
  | { type: 'setVolume'; percent: number }
  | { type: 'setLoopMode'; mode: LoopMode }
  | { type: 'setAutoplay'; enabled: boolean }
  | { type: 'setStay247'; enabled: boolean }
  | { type: 'setHrir'; mode: AuraToggle }
  | { type: 'setAura360'; mode: AuraToggle }
  | { type: 'setAuraPreset'; id: string }
  | { type: 'removeQueueItem'; id: string }
  | { type: 'moveQueueItem'; id: string; toIndex: number }
  | { type: 'clearQueue' }
  | { type: 'addTrack'; query: string }
  | { type: 'loadPlaylist'; sourceUrls: string[] };

export interface CommandResult {
  ok: boolean;
  /** User-safe error message (Japanese) when ok is false. */
  error?: string;
  /** Fresh snapshot after the command succeeded, for immediate UI update (null if the session ended, e.g. stop). */
  snapshot?: GuildSnapshot | null;
}

/**
 * The single seam between the HTTP layer and the bot. Implemented by LocalBridge
 * (single process, direct calls) and ShardedBridge (manager process, broadcastEval
 * + shard IPC). Every method takes the authenticated userId; nothing about the
 * caller is trusted beyond that — authorization is re-checked on the owning shard.
 */
export interface BotBridge {
  /** The subset of the viewer's guilds (from the OAuth `guilds` scope) that currently have a bot session. */
  listControllableGuilds(userGuildIds: string[], userId: string): Promise<GuildSummary[]>;
  /** null when the guild has no active player (or isn't on any shard yet). Recomputes viewer capabilities. */
  getSnapshot(guildId: string, userId: string): Promise<GuildSnapshot | null>;
  /** Executes a command after re-authorizing on the owning shard. Never trusts the caller beyond userId. */
  runCommand(guildId: string, userId: string, command: WebCommand): Promise<CommandResult>;
  /** Runs a YouTube search (guild-independent) and returns candidates without enqueuing. */
  search(query: string): Promise<SearchCandidate[]>;
  /**
   * Realtime. `cb` fires with a fresh snapshot on every throttled player update,
   * and once with null when the session is destroyed. Returns an unsubscribe
   * function. `userId` lets the single-process bridge compute live per-viewer
   * capabilities; the sharded bridge fans one shard push out to all subscribers,
   * so its pushes carry a display-only `viewer` (control always re-authorizes on
   * write through runCommand regardless).
   */
  subscribe(guildId: string, userId: string, cb: (snapshot: GuildSnapshot | null) => void): () => void;
  /** Tears down listeners/IPC handlers. */
  close(): void;
}
