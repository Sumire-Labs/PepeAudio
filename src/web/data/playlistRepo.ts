import { randomUUID } from 'node:crypto';
import { webDb } from './webDb.js';
import type { SourceType } from '../bridge/types.js';

export const MAX_PLAYLISTS_PER_USER = 25;
export const MAX_TRACKS_PER_PLAYLIST = 50;
const MAX_NAME_LENGTH = 100;
const MAX_FIELD_LENGTH = 500;

const SOURCE_TYPES = new Set<SourceType>(['youtube', 'spotify', 'soundcloud', 'applemusic']);

export interface PlaylistTrackDTO {
  sourceUrl: string;
  title: string;
  artist: string;
  thumbnailUrl: string | null;
  sourceType: SourceType;
  durationMs: number | null;
}

export interface PlaylistSummary {
  id: string;
  name: string;
  trackCount: number;
  updatedAt: number;
}

export interface PlaylistDetail extends PlaylistSummary {
  tracks: PlaylistTrackDTO[];
}

export type RepoResult = { ok: true } | { error: string };

function sanitizeName(name: unknown): string | null {
  if (typeof name !== 'string') return null;
  // Strip ASCII control chars (C0 + DEL) so a name can't smuggle newlines/escapes; by codepoint to avoid a control-char regex.
  let cleaned = '';
  for (const ch of name) {
    const code = ch.codePointAt(0) ?? 0;
    if (code < 0x20 || code === 0x7f) continue;
    cleaned += ch;
  }
  cleaned = cleaned.trim().slice(0, MAX_NAME_LENGTH);
  return cleaned.length > 0 ? cleaned : null;
}

function isValidTrack(track: unknown): track is PlaylistTrackDTO {
  if (!track || typeof track !== 'object') return false;
  const t = track as Record<string, unknown>;
  // sourceUrl may be a provider URL or a plain "artist title" search string; both go only to resolveInput (SSRF-guarded by classifyInput).
  if (typeof t.sourceUrl !== 'string' || t.sourceUrl.trim().length === 0 || t.sourceUrl.length > MAX_FIELD_LENGTH) return false;
  if (typeof t.title !== 'string' || t.title.length > MAX_FIELD_LENGTH) return false;
  if (typeof t.artist !== 'string' || t.artist.length > MAX_FIELD_LENGTH) return false;
  if (typeof t.sourceType !== 'string' || !SOURCE_TYPES.has(t.sourceType as SourceType)) return false;
  if (t.thumbnailUrl !== null && typeof t.thumbnailUrl !== 'string') return false;
  if (t.durationMs !== null && typeof t.durationMs !== 'number') return false;
  return true;
}

// Collection (playlist/album/set) URLs are shared by every track in the collection, so persisting one verbatim would re-import the whole collection on load.
function looksLikeCollection(url: string): boolean {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return false;
  }
  const host = parsed.hostname.toLowerCase();
  const path = parsed.pathname.toLowerCase();
  if (host === 'youtube.com' || host.endsWith('.youtube.com') || host === 'youtu.be') {
    if (path.startsWith('/playlist')) return true;
    return parsed.searchParams.has('list') && !parsed.searchParams.has('v');
  }
  if (host === 'spotify.com' || host.endsWith('.spotify.com')) return /\/(playlist|album)\//.test(path);
  if (host === 'music.apple.com') return path.includes('/playlist/') || (path.includes('/album/') && !parsed.searchParams.has('i'));
  if (host === 'soundcloud.com' || host.endsWith('.soundcloud.com')) return path.includes('/sets/');
  return false;
}

// Rewrites a collection sourceUrl to an "artist title" search string; idempotent for per-track URLs and existing search strings.
function normalizeTrackForSave(track: PlaylistTrackDTO): PlaylistTrackDTO {
  if (!looksLikeCollection(track.sourceUrl)) return track;
  const query = (`${track.artist} ${track.title}`.trim() || track.title.trim()).slice(0, MAX_FIELD_LENGTH);
  // Keep the original if there's nothing to search by — an empty string would be rejected by isValidTrack on reload.
  return query.length > 0 ? { ...track, sourceUrl: query } : track;
}

interface PlaylistRow {
  id: string;
  name: string;
  userId: string;
  updatedAt: number;
}

export class PlaylistRepo {
  private readonly selectByUser = webDb.prepare(
    `SELECT p.id, p.name, p.updated_at AS updatedAt,
            (SELECT COUNT(*) FROM web_playlist_tracks t WHERE t.playlist_id = p.id) AS trackCount
     FROM web_playlists p WHERE p.user_id = ? ORDER BY p.updated_at DESC`,
  );
  private readonly selectOne = webDb.prepare(
    `SELECT id, name, user_id AS userId, updated_at AS updatedAt FROM web_playlists WHERE id = ?`,
  );
  private readonly selectTracks = webDb.prepare(
    `SELECT source_url AS sourceUrl, title, artist, thumbnail_url AS thumbnailUrl,
            source_type AS sourceType, duration_ms AS durationMs
     FROM web_playlist_tracks WHERE playlist_id = ? ORDER BY position ASC`,
  );
  private readonly countByUser = webDb.prepare(`SELECT COUNT(*) AS c FROM web_playlists WHERE user_id = ?`);
  private readonly insertPlaylist = webDb.prepare(
    `INSERT INTO web_playlists (id, user_id, name, created_at, updated_at) VALUES (?, ?, ?, ?, ?)`,
  );
  private readonly updateName = webDb.prepare(`UPDATE web_playlists SET name = ?, updated_at = ? WHERE id = ? AND user_id = ?`);
  private readonly touch = webDb.prepare(`UPDATE web_playlists SET updated_at = ? WHERE id = ?`);
  private readonly deletePlaylistStmt = webDb.prepare(`DELETE FROM web_playlists WHERE id = ? AND user_id = ?`);
  private readonly deleteTracksStmt = webDb.prepare(`DELETE FROM web_playlist_tracks WHERE playlist_id = ?`);
  private readonly insertTrack = webDb.prepare(
    `INSERT INTO web_playlist_tracks (playlist_id, position, source_url, title, artist, thumbnail_url, source_type, duration_ms)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
  );

  list(userId: string): PlaylistSummary[] {
    return this.selectByUser.all(userId) as PlaylistSummary[];
  }

  get(userId: string, id: string): PlaylistDetail | null {
    const row = this.selectOne.get(id) as PlaylistRow | undefined;
    if (!row || row.userId !== userId) return null;
    const tracks = this.selectTracks.all(id) as PlaylistTrackDTO[];
    return { id: row.id, name: row.name, updatedAt: row.updatedAt, trackCount: tracks.length, tracks };
  }

  create(userId: string, name: unknown): PlaylistSummary | { error: string } {
    const count = (this.countByUser.get(userId) as { c: number }).c;
    if (count >= MAX_PLAYLISTS_PER_USER) return { error: `プレイリストは最大 ${MAX_PLAYLISTS_PER_USER} 個までです。` };
    const cleanName = sanitizeName(name);
    if (!cleanName) return { error: '名前を入力してください。' };
    const id = randomUUID();
    const now = Date.now();
    this.insertPlaylist.run(id, userId, cleanName, now, now);
    return { id, name: cleanName, trackCount: 0, updatedAt: now };
  }

  rename(userId: string, id: string, name: unknown): boolean {
    const cleanName = sanitizeName(name);
    if (!cleanName) return false;
    return this.updateName.run(cleanName, Date.now(), id, userId).changes > 0;
  }

  delete(userId: string, id: string): boolean {
    const res = this.deletePlaylistStmt.run(id, userId);
    if (res.changes > 0) this.deleteTracksStmt.run(id);
    return res.changes > 0;
  }

  /** Replaces the entire ordered track list (reorder/remove). Transactional. */
  replaceTracks(userId: string, id: string, tracks: unknown): RepoResult {
    const row = this.selectOne.get(id) as PlaylistRow | undefined;
    if (!row || row.userId !== userId) return { error: 'プレイリストが見つかりません。' };
    if (!Array.isArray(tracks)) return { error: '不正なリクエストです。' };
    const clean = tracks.filter(isValidTrack).map(normalizeTrackForSave).slice(0, MAX_TRACKS_PER_PLAYLIST);
    const tx = webDb.transaction((items: PlaylistTrackDTO[]) => {
      this.deleteTracksStmt.run(id);
      items.forEach((t, i) =>
        this.insertTrack.run(id, i, t.sourceUrl, t.title, t.artist, t.thumbnailUrl ?? null, t.sourceType, t.durationMs ?? null),
      );
      this.touch.run(Date.now(), id);
    });
    tx(clean);
    return { ok: true };
  }

  addTrack(userId: string, id: string, track: unknown): RepoResult {
    const detail = this.get(userId, id);
    if (!detail) return { error: 'プレイリストが見つかりません。' };
    if (!isValidTrack(track)) return { error: '不正なトラックです。' };
    if (detail.trackCount >= MAX_TRACKS_PER_PLAYLIST) return { error: `プレイリストは最大 ${MAX_TRACKS_PER_PLAYLIST} 曲までです。` };
    const t = normalizeTrackForSave(track);
    this.insertTrack.run(
      id,
      detail.trackCount, // positions are dense, so the count is the next index
      t.sourceUrl,
      t.title,
      t.artist,
      t.thumbnailUrl ?? null,
      t.sourceType,
      t.durationMs ?? null,
    );
    this.touch.run(Date.now(), id);
    return { ok: true };
  }

  /** Appends many tracks (import from URL); silently caps the batch to the remaining room. */
  addTracks(userId: string, id: string, tracks: unknown): { ok: true; added: number } | { error: string } {
    const detail = this.get(userId, id);
    if (!detail) return { error: 'プレイリストが見つかりません。' };
    if (!Array.isArray(tracks)) return { error: '不正なリクエストです。' };
    const remaining = MAX_TRACKS_PER_PLAYLIST - detail.trackCount;
    if (remaining <= 0) return { error: `プレイリストは最大 ${MAX_TRACKS_PER_PLAYLIST} 曲までです。` };
    const clean = tracks.filter(isValidTrack).map(normalizeTrackForSave).slice(0, remaining);
    if (clean.length === 0) return { error: '追加できる曲がありませんでした。' };
    const base = detail.trackCount;
    const tx = webDb.transaction((items: PlaylistTrackDTO[]) => {
      items.forEach((t, i) =>
        this.insertTrack.run(id, base + i, t.sourceUrl, t.title, t.artist, t.thumbnailUrl ?? null, t.sourceType, t.durationMs ?? null),
      );
      this.touch.run(Date.now(), id);
    });
    tx(clean);
    return { ok: true, added: clean.length };
  }
}
