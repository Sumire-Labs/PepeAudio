import { fetchSnapshot } from "./api-client";
import type { AuthGuild } from "./auth-wire";
import { selectInitialGuild } from "./live-dashboard-model";
import type { PlayerSnapshotWire } from "./wire-types";

const PROBE_BATCH_SIZE = 6;

type SnapshotProbe = (
  guildId: string,
  signal?: AbortSignal
) => Promise<PlayerSnapshotWire>;

export function findPreferredGuild(
  currentGuildId: string,
  guilds: readonly AuthGuild[],
  userId: string | null,
  signal: AbortSignal
): Promise<string> {
  return findPreferredGuildWith(
    currentGuildId,
    guilds,
    userId,
    signal,
    fetchSnapshot
  );
}

export async function findPreferredGuildWith(
  currentGuildId: string,
  guilds: readonly AuthGuild[],
  userId: string | null,
  signal: AbortSignal,
  probe: SnapshotProbe
): Promise<string> {
  const fallback = selectInitialGuild(currentGuildId, guilds);
  const candidates = guilds.filter((guild) => guild.botPresent);

  for (let offset = 0; offset < candidates.length; offset += PROBE_BATCH_SIZE) {
    if (signal.aborted) return fallback;
    const batch = candidates.slice(offset, offset + PROBE_BATCH_SIZE);
    const results = await Promise.all(batch.map(async (guild) => {
      try {
        const snapshot = await probe(guild.id, signal);
        return { guildId: guild.id, score: snapshotScore(snapshot, userId) };
      } catch {
        return { guildId: guild.id, score: 0 };
      }
    }));
    const requested = results.find((result) => result.score === 2);
    if (requested) return requested.guildId;
    const active = results.find((result) => result.score === 1);
    if (active) return active.guildId;
  }

  return fallback;
}

function snapshotScore(snapshot: PlayerSnapshotWire, userId: string | null): 0 | 1 | 2 {
  const activelyPlaying = snapshot.voice_channel_id !== null
    && snapshot.current_track !== null
    && (snapshot.state === "playing"
      || snapshot.state === "paused"
      || snapshot.state === "loading");
  if (!activelyPlaying) return 0;
  if (userId !== null && (
    snapshot.current_track?.requester_user_id === userId
    || snapshot.upcoming_tracks.some((track) => track.requester_user_id === userId)
  )) {
    return 2;
  }
  return 1;
}
