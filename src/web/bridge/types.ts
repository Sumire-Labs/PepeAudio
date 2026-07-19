/**
 * Types only, no runtime imports, so this can be imported from any process
 * (the ShardingManager included — it has no discord.js Client). Shapes must be
 * JSON-serializable to survive structuredClone across the shard IPC boundary:
 * no functions, class instances, or Maps.
 */

export type SourceType = 'youtube' | 'spotify' | 'soundcloud' | 'applemusic';
export type LoopMode = 'off' | 'track' | 'queue';
export type AuraToggle = 'off' | 'on';
export type PermissionMode = 'same-voice-channel' | 'dj-role' | 'requester-only';

/** QueueItem without its functions (getStream/prefetch) or internal offset. */
export interface QueueItemDTO {
  id: string;
  title: string;
  artist: string;
  durationMs: number | null;
  thumbnailUrl: string | null;
  sourceType: SourceType;
  sourceUrl: string;
  /** Discord user id of who requested it. */
  requestedBy: string;
  /** Resolved requester display name (from the guild member cache), or null if uncached. */
  requesterName: string | null;
  /** Resolved requester avatar URL, or null if uncached. */
  requesterAvatarUrl: string | null;
}

export interface ViewerCapabilities {
  canControl: boolean;
  /** User-safe reason (Japanese) when canControl is false. */
  denyReason: string | null;
  inBotVoiceChannel: boolean;
}

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
  auraPresets: Array<{ id: string; label: string }>;
  stay247: boolean;
  permissionMode: PermissionMode;
  voiceChannelId: string;
  lastError: string | null;
  /** Whether the Aura (3D audio) feature is enabled at all in this build. */
  auraEnabled: boolean;
  viewer: ViewerCapabilities;
}

export interface SearchCandidate {
  title: string;
  author: string;
  url: string;
  thumbnailUrl: string;
}

export interface ResolveResult {
  tracks: QueueItemDTO[];
  /** User-safe error (Japanese) when resolution failed. */
  error?: string;
}

export interface GuildSummary {
  guildId: string;
  name: string;
  iconUrl: string | null;
  hasActiveSession: boolean;
  status: 'idle' | 'playing' | 'paused';
  currentTitle: string | null;
}

/**
 * Every state-changing operation the browser can request. Numeric/enum payloads
 * are re-validated server-side before touching the player.
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
  | { type: 'jumpTo'; id: string }
  | { type: 'seek'; positionMs: number }
  | { type: 'clearQueue' }
  | { type: 'addTrack'; query: string }
  | { type: 'loadPlaylist'; sourceUrls: string[] };

export interface CommandResult {
  ok: boolean;
  /** User-safe error message (Japanese) when ok is false. */
  error?: string;
  /** Fresh snapshot after success; null if the session ended (e.g. stop). */
  snapshot?: GuildSnapshot | null;
}

/**
 * The seam between the HTTP layer and the bot (LocalBridge = single process,
 * ShardedBridge = manager + shard IPC). Every method takes the authenticated
 * userId; the caller is not trusted beyond that — authorization is re-checked
 * on the owning shard.
 */
export interface BotBridge {
  /** The subset of the viewer's OAuth `guilds` that currently have a bot session. */
  listControllableGuilds(userGuildIds: string[], userId: string): Promise<GuildSummary[]>;
  /** null when the guild has no active player (or isn't on any shard yet). Recomputes viewer capabilities. */
  getSnapshot(guildId: string, userId: string): Promise<GuildSnapshot | null>;
  /** Executes a command after re-authorizing on the owning shard. Never trusts the caller beyond userId. */
  runCommand(guildId: string, userId: string, command: WebCommand): Promise<CommandResult>;
  /** Runs a YouTube search (guild-independent) and returns candidates without enqueuing. */
  search(query: string): Promise<SearchCandidate[]>;
  /** Resolves a URL (or search) to tracks WITHOUT enqueuing. */
  resolveTracks(query: string): Promise<ResolveResult>;
  /**
   * `cb` fires with a fresh snapshot on every throttled player update, and once
   * with null when the session is destroyed; returns an unsubscribe function.
   * The sharded bridge fans one push out to all subscribers, so its `viewer` is
   * display-only — control always re-authorizes on write via runCommand.
   */
  subscribe(guildId: string, userId: string, cb: (snapshot: GuildSnapshot | null) => void): () => void;
  close(): void;
}
