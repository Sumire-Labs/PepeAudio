import type { PlayerSnapshotWire, TrackSnapshotWire } from "./wire-types";
import { parsePublicMediaPage } from "./public-media-page";

export type PlayerSseAction =
  | { readonly kind: "ignore" }
  | { readonly kind: "resync" }
  | { readonly kind: "snapshot"; readonly snapshot: PlayerSnapshotWire };

export function interpretPlayerSseFrame(
  frameText: string,
  expectedGuildId: string,
  currentRevision: number
): PlayerSseAction {
  const frame = parseSseFrame(frameText);
  if (frame === null) return { kind: "ignore" };
  if (frame.event === "resync") return { kind: "resync" };
  if (frame.event !== "snapshot" && frame.event !== "player") return { kind: "resync" };

  try {
    const payload = record(JSON.parse(frame.data) as unknown);
    const snapshot = parsePlayerSnapshotWire(payload.snapshot, expectedGuildId);
    if (payload.revision !== snapshot.revision) return { kind: "resync" };
    if (snapshot.revision <= currentRevision) return { kind: "ignore" };
    if (frame.event === "player" && snapshot.revision !== currentRevision + 1) {
      return { kind: "resync" };
    }
    return { kind: "snapshot", snapshot };
  } catch {
    return { kind: "resync" };
  }
}

export function parseSseFrame(frame: string): { event: string; data: string } | null {
  let event = "message";
  const data: string[] = [];
  for (const line of frame.split("\n")) {
    if (line.startsWith("event:")) event = line.slice(6).trimStart();
    if (line.startsWith("data:")) data.push(line.slice(5).trimStart());
  }
  return data.length === 0 ? null : { event, data: data.join("\n") };
}

export function parsePlayerSnapshotWire(
  value: unknown,
  expectedGuildId: string
): PlayerSnapshotWire {
  const snapshot = record(value);
  if (snapshot.guild_id !== expectedGuildId || !snowflake(snapshot.guild_id)) invalid();
  if (snapshot.voice_channel_id !== null && !snowflake(snapshot.voice_channel_id)) invalid();
  if (!whole(snapshot.revision) || !whole(snapshot.queued_tracks)) invalid();
  if (!playerState(snapshot.state) || !repeatMode(snapshot.repeat_mode)) invalid();
  if (!Array.isArray(snapshot.upcoming_tracks) || snapshot.upcoming_tracks.length > 100) invalid();
  if (!whole(snapshot.volume) || snapshot.volume > 100 || !whole(snapshot.observed_at)) invalid();
  if (!boolean(snapshot.has_previous_track) || !boolean(snapshot.shuffle_enabled)) invalid();
  if (!boolean(snapshot.spatial_audio_enabled)) invalid();
  if (snapshot.hrir_preset !== null && !boundedText(snapshot.hrir_preset, 128)) invalid();

  const current = snapshot.current_track === null ? null : track(snapshot.current_track);
  const upcoming = snapshot.upcoming_tracks.map(track);
  if (snapshot.queued_tracks !== upcoming.length) invalid();
  const trackIds = new Set(upcoming.map((track) => track.track_id));
  if (trackIds.size !== upcoming.length ||
      (current !== null && trackIds.has(current.track_id))) invalid();
  return {
    ...snapshot,
    current_track: current,
    upcoming_tracks: upcoming
  } as unknown as PlayerSnapshotWire;
}

function track(value: unknown): TrackSnapshotWire {
  const candidate = record(value);
  if (!uuid(candidate.track_id) || !boundedUtf8Text(candidate.title, 512)) invalid();
  const artist = optionalText(candidate.artist, 256);
  const album = optionalText(candidate.album, 512);
  const provenance = candidate.provenance === undefined || candidate.provenance === null
    ? null
    : trackProvenance(candidate.provenance);
  if (candidate.requester_user_id !== null && !snowflake(candidate.requester_user_id)) invalid();
  if (candidate.duration_ms !== null && !whole(candidate.duration_ms)) invalid();
  if (!whole(candidate.position_ms) || !boolean(candidate.seekable)) invalid();
  if (candidate.duration_ms !== null && candidate.position_ms > candidate.duration_ms) invalid();
  return {
    ...candidate,
    artist,
    album,
    provenance
  } as unknown as TrackSnapshotWire;
}

function trackProvenance(value: unknown): NonNullable<TrackSnapshotWire["provenance"]> {
  const candidate = record(value);
  const origin = candidate.origin === undefined || candidate.origin === null
    ? null
    : parsePublicMediaPage(candidate.origin);
  const playback = parsePublicMediaPage(candidate.playback);
  if (playback.provider !== "youtube" && playback.provider !== "soundcloud") invalid();
  return { origin, playback };
}

function optionalText(value: unknown, limit: number): string | null {
  if (value === undefined || value === null) return null;
  if (!boundedUtf8Text(value, limit)) invalid();
  return value;
}

function record(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) invalid();
  return value as Record<string, unknown>;
}

function whole(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

function boolean(value: unknown): value is boolean {
  return typeof value === "boolean";
}

function boundedText(value: unknown, limit: number): value is string {
  return typeof value === "string" && value.length > 0 && value.length <= limit;
}

function boundedUtf8Text(value: unknown, limit: number): value is string {
  return typeof value === "string" && value.length > 0 &&
    new TextEncoder().encode(value).length <= limit &&
    ![...value].some((character) => /\p{Cc}/u.test(character));
}

function snowflake(value: unknown): value is string {
  return typeof value === "string" && /^[1-9][0-9]{0,19}$/u.test(value) &&
    (value.length < 20 || value <= "18446744073709551615");
}

function uuid(value: unknown): value is string {
  return typeof value === "string" &&
    /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/iu.test(value);
}

function playerState(value: unknown): boolean {
  return value === "disconnected" || value === "idle_connected" || value === "loading" ||
    value === "playing" || value === "paused";
}

function repeatMode(value: unknown): boolean {
  return value === "off" || value === "track" || value === "queue";
}

function invalid(): never {
  throw new Error("Player snapshot is invalid");
}
