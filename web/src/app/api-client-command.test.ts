import { describe, expect, it, vi } from "vitest";

import {
  ApiResponseError,
  sendPlayerCommand,
  waitForCommandResult,
  waitForSnapshotRevision
} from "./api-client";
import type { CommandResultWire } from "./command-result";
import type { PlayerSnapshotWire } from "./wire-types";

const COMMAND_ID = "00000000-0000-0000-0000-000000000001";
const IDEMPOTENCY_KEY = "00000000-0000-0000-0000-000000000002";
const GUILD_ID = "123";

describe("command correlation", () => {
  it("returns the accepted command receipt instead of discarding it", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({
      command_id: COMMAND_ID,
      idempotency_key: IDEMPOTENCY_KEY,
      resulting_revision: null,
      replayed: false
    }), {
      status: 202,
      headers: { "content-type": "application/json" }
    })));

    await expect(sendPlayerCommand(
      GUILD_ID,
      4,
      { type: "pause" },
      "csrf-token"
    )).resolves.toMatchObject({ command_id: COMMAND_ID });
  });

  it("polls the exact command until its terminal result", async () => {
    let clock = 0;
    const results: CommandResultWire[] = [
      { command_id: COMMAND_ID, guild_id: GUILD_ID, status: "pending" },
      {
        command_id: COMMAND_ID,
        guild_id: GUILD_ID,
        status: "applied",
        resulting_revision: 9
      }
    ];
    const fetchResult = vi.fn(async () => results.shift()!);

    await expect(waitForCommandResult(GUILD_ID, COMMAND_ID, 1_000, {
      fetchResult,
      now: () => clock,
      wait: async (milliseconds) => { clock += milliseconds; }
    })).resolves.toMatchObject({ status: "applied", resulting_revision: 9 });
    expect(fetchResult).toHaveBeenCalledTimes(2);
  });

  it("fails closed when the correlated result has expired", async () => {
    await expect(waitForCommandResult(GUILD_ID, COMMAND_ID, 1_000, {
      fetchResult: async () => {
        throw new ApiResponseError(404, "not found");
      }
    })).rejects.toThrow("成功したものとして扱わず");
  });

  it("waits for the applied revision rather than any unrelated revision increase", async () => {
    let clock = 0;
    const snapshots = [snapshot(8), snapshot(9)];
    const fetchCurrent = vi.fn(async () => snapshots.shift()!);

    const confirmed = await waitForSnapshotRevision(GUILD_ID, 9, 1_000, {
      fetchCurrent,
      now: () => clock,
      wait: async (milliseconds) => { clock += milliseconds; }
    });

    expect(confirmed.revision).toBe(9);
    expect(fetchCurrent).toHaveBeenCalledTimes(2);
  });
});

function snapshot(revision: number): PlayerSnapshotWire {
  return {
    guild_id: GUILD_ID,
    voice_channel_id: null,
    revision,
    state: "idle_connected",
    current_track: null,
    queued_tracks: 0,
    upcoming_tracks: [],
    has_previous_track: false,
    volume: 75,
    repeat_mode: "off",
    shuffle_enabled: false,
    hrir_preset: null,
    spatial_audio_enabled: false,
    observed_at: 1
  };
}
