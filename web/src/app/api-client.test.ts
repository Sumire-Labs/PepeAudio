import { describe, expect, it, vi } from "vitest";

import {
  interpretPlayerSseFrame,
  maintainPlayerEventStream,
  parseHrirPresetCatalog,
  parseSseFrame,
  sseReconnectDelayMs,
  streamPlayerEvents
} from "./api-client";
import type { PlayerSnapshotWire } from "./wire-types";

describe("parseSseFrame", () => {
  it("parses named events and joins data lines", () => {
    expect(parseSseFrame("id: 4\nevent: player\ndata: {\"a\":\ndata: 1}" )).toEqual({
      event: "player",
      data: "{\"a\":\n1}"
    });
  });

  it("ignores keepalive-only frames", () => {
    expect(parseSseFrame(": keepalive")).toBeNull();
  });
});

describe("player SSE recovery", () => {
  it("maps an expired streaming session to a safe authentication error", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({
      error: { code: "authentication_required", message: "private detail" }
    }), {
      status: 401,
      headers: { "content-type": "application/json" }
    })));

    await expect(streamPlayerEvents(
      "123",
      0,
      new AbortController().signal,
      () => undefined
    )).rejects.toMatchObject({
      status: 401,
      message: "Discordでログインし直してください。"
    });
  });

  it("requires a resync for revision gaps and invalid events", () => {
    const guildId = "123";
    expect(interpretPlayerSseFrame(eventFrame("player", snapshot(guildId, 3)), guildId, 1))
      .toEqual({ kind: "resync" });
    expect(interpretPlayerSseFrame("event: player\ndata: {broken", guildId, 1))
      .toEqual({ kind: "resync" });
    expect(interpretPlayerSseFrame("event: resync\ndata: {}", guildId, 1))
      .toEqual({ kind: "resync" });
    const mismatchedQueue = { ...snapshot(guildId, 2), queued_tracks: 1 };
    expect(interpretPlayerSseFrame(eventFrame("player", mismatchedQueue), guildId, 1))
      .toEqual({ kind: "resync" });

    const oversizedQueue = {
      ...snapshot(guildId, 2),
      queued_tracks: 101,
      upcoming_tracks: Array.from({ length: 101 }, (_, index) => track(index))
    };
    expect(interpretPlayerSseFrame(eventFrame("player", oversizedQueue), guildId, 1))
      .toEqual({ kind: "resync" });

    const oversizedUtf8Title = {
      ...snapshot(guildId, 2),
      current_track: { ...track(1), title: "音".repeat(171) }
    };
    expect(interpretPlayerSseFrame(eventFrame("player", oversizedUtf8Title), guildId, 1))
      .toEqual({ kind: "resync" });

    const duplicateTrack = track(1);
    const duplicateQueue = {
      ...snapshot(guildId, 2),
      current_track: duplicateTrack,
      queued_tracks: 1,
      upcoming_tracks: [duplicateTrack]
    };
    expect(interpretPlayerSseFrame(eventFrame("player", duplicateQueue), guildId, 1))
      .toEqual({ kind: "resync" });

    const impossiblePosition = {
      ...snapshot(guildId, 2),
      current_track: { ...track(1), duration_ms: 1_000, position_ms: 1_001 }
    };
    expect(interpretPlayerSseFrame(eventFrame("player", impossiblePosition), guildId, 1))
      .toEqual({ kind: "resync" });
  });

  it("measures the streaming frame limit in UTF-8 bytes", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response(
      `data: ${"音".repeat(350_000)}`,
      { status: 200, headers: { "content-type": "text/event-stream" } }
    )));

    await expect(streamPlayerEvents(
      "123",
      0,
      new AbortController().signal,
      () => undefined
    )).rejects.toThrow("frame exceeded");
  });

  it("limits each SSE frame instead of the combined network chunk", async () => {
    const keepalive = `:${"x".repeat(600_000)}\n\n`;
    vi.stubGlobal("fetch", vi.fn(async () => new Response(
      keepalive + keepalive,
      { status: 200, headers: { "content-type": "text/event-stream" } }
    )));

    await expect(streamPlayerEvents(
      "123",
      0,
      new AbortController().signal,
      () => undefined
    )).resolves.toBeUndefined();
  });

  it("accepts a full snapshot jump and one contiguous player mutation", () => {
    const guildId = "123";
    expect(interpretPlayerSseFrame(eventFrame("snapshot", snapshot(guildId, 4)), guildId, 1))
      .toEqual({ kind: "snapshot", snapshot: snapshot(guildId, 4) });
    expect(interpretPlayerSseFrame(eventFrame("player", snapshot(guildId, 2)), guildId, 1))
      .toEqual({ kind: "snapshot", snapshot: snapshot(guildId, 2) });
  });

  it("uses bounded exponential backoff with jitter", () => {
    expect(sseReconnectDelayMs(0, 0)).toBe(125);
    expect(sseReconnectDelayMs(0, 1)).toBe(250);
    expect(sseReconnectDelayMs(2, 0)).toBe(500);
    expect(sseReconnectDelayMs(100, 1)).toBe(10_000);
    expect(sseReconnectDelayMs(100, 0)).toBe(5_000);
  });

  it("fetches a fresh snapshot before every sequential stream and stops on abort", async () => {
    const guildId = "123";
    const controller = new AbortController();
    const fetched = [snapshot(guildId, 1), snapshot(guildId, 2)];
    const applied: number[] = [];
    let openCalls = 0;
    let activeStreams = 0;
    let maximumActiveStreams = 0;
    const fetchCurrent = vi.fn(async () => fetched.shift()!);
    const openStream = vi.fn(async () => {
      openCalls += 1;
      activeStreams += 1;
      maximumActiveStreams = Math.max(maximumActiveStreams, activeStreams);
      activeStreams -= 1;
      if (openCalls === 2) controller.abort();
    });
    const wait = vi.fn(async () => true);

    await maintainPlayerEventStream(
      guildId,
      controller.signal,
      (wire) => applied.push(wire.revision),
      () => undefined,
      { fetchCurrent, openStream, random: () => 0, wait }
    );

    expect(fetchCurrent).toHaveBeenCalledTimes(2);
    expect(openStream).toHaveBeenCalledTimes(2);
    expect(applied).toEqual([1, 2]);
    expect(maximumActiveStreams).toBe(1);
    expect(wait).toHaveBeenCalledTimes(1);
  });
});

describe("parseHrirPresetCatalog", () => {
  const guildId = "18446744073709551615";

  it("preserves string snowflakes and public source metadata", () => {
    expect(parseHrirPresetCatalog({
      guild_id: guildId,
      presets: [{
        id: "studio-neutral",
        display_name: "Studio Neutral",
        description: "Balanced room response",
        source: {
          license_name: "CC0-1.0",
          source_url: "https://example.test/source",
          attribution: "Fixture author"
        }
      }]
    }, guildId)).toEqual([{
      id: "studio-neutral",
      name: "Studio Neutral",
      description: "Balanced room response",
      source: {
        licenseName: "CC0-1.0",
        sourceUrl: "https://example.test/source",
        attribution: "Fixture author"
      }
    }]);
  });

  it("accepts an honestly empty catalog", () => {
    expect(parseHrirPresetCatalog({ guild_id: guildId, presets: [] }, guildId)).toEqual([]);
  });

  it("rejects guild mismatches, duplicate IDs, and unsafe source URLs", () => {
    expect(() => parseHrirPresetCatalog({ guild_id: "10", presets: [] }, guildId)).toThrow(
      "guild mismatch"
    );
    const duplicate = {
      guild_id: guildId,
      presets: [
        { id: "same", display_name: "One", source: {} },
        { id: "same", display_name: "Two", source: {} }
      ]
    };
    expect(() => parseHrirPresetCatalog(duplicate, guildId)).toThrow("duplicate IDs");
    expect(() => parseHrirPresetCatalog({
      guild_id: guildId,
      presets: [{
        id: "unsafe",
        display_name: "Unsafe",
        source: { source_url: "javascript:alert(1)" }
      }]
    }, guildId)).toThrow("source URL is invalid");

    expect(() => parseHrirPresetCatalog({
      guild_id: guildId,
      presets: [{
        id: "unsafe-description",
        display_name: "Unsafe",
        description: "line one\nline two",
        source: {}
      }]
    }, guildId)).toThrow("description is invalid");
  });

  it("accepts a missing optional description", () => {
    expect(parseHrirPresetCatalog({
      guild_id: guildId,
      presets: [{ id: "plain", display_name: "Plain", source: {} }]
    }, guildId)[0]?.description).toBeNull();
  });
});

function eventFrame(kind: "snapshot" | "player", wire: PlayerSnapshotWire): string {
  return `event: ${kind}\ndata: ${JSON.stringify({ revision: wire.revision, snapshot: wire })}`;
}

function snapshot(guildId: string, revision: number): PlayerSnapshotWire {
  return {
    guild_id: guildId,
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

function track(index: number): NonNullable<PlayerSnapshotWire["current_track"]> {
  return {
    track_id: `00000000-0000-0000-0000-${(index + 1).toString().padStart(12, "0")}`,
    title: `Track ${index + 1}`,
    artist: null,
    album: null,
    provenance: null,
    requester_user_id: null,
    duration_ms: 180_000,
    position_ms: 0,
    seekable: true
  };
}
