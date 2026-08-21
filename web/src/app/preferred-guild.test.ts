import { describe, expect, it, vi } from "vitest";

import type { AuthGuild } from "./auth-wire";
import { findPreferredGuildWith } from "./preferred-guild";
import type { PlayerSnapshotWire } from "./wire-types";

const guilds: readonly AuthGuild[] = [guild("1"), guild("2"), guild("3")];

describe("findPreferredGuildWith", () => {
  it("prefers an active player requested by the signed-in user", async () => {
    const probe = vi.fn(async (guildId: string) => snapshot(
      guildId,
      guildId === "2" ? "playing" : "disconnected",
      guildId === "2" ? "99" : null
    ));

    await expect(findPreferredGuildWith(
      "",
      guilds,
      "99",
      new AbortController().signal,
      probe
    )).resolves.toBe("2");
  });

  it("falls back to the first installed guild when no player is active", async () => {
    await expect(findPreferredGuildWith(
      "",
      guilds,
      "99",
      new AbortController().signal,
      async (guildId) => snapshot(guildId, "disconnected", null)
    )).resolves.toBe("1");
  });
});

function guild(id: string): AuthGuild {
  return {
    id,
    name: `Guild ${id}`,
    icon: null,
    owner: false,
    permissions: "0",
    botPresent: true
  };
}

function snapshot(
  guildId: string,
  state: PlayerSnapshotWire["state"],
  requesterUserId: string | null
): PlayerSnapshotWire {
  const current = state === "disconnected" ? null : {
    track_id: "84cb4cf6-7e0a-4c5e-b44b-cb8d8df5d37d",
    title: "Track",
    requester_user_id: requesterUserId,
    duration_ms: 60_000,
    position_ms: 0,
    seekable: true
  };
  return {
    guild_id: guildId,
    voice_channel_id: current === null ? null : "10",
    revision: 1,
    state,
    current_track: current,
    queued_tracks: 0,
    upcoming_tracks: [],
    has_previous_track: false,
    volume: 10,
    repeat_mode: "off",
    shuffle_enabled: false,
    hrir_preset: "dht",
    spatial_audio_enabled: true,
    observed_at: Date.now()
  };
}
