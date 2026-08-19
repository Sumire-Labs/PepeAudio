import { describe, expect, it } from "vitest";

import { createDemoSnapshot } from "./demo-data";
import {
  formatDuration,
  interpolatedPositionMs,
  orbitDegreesAt,
  progressPercent
} from "./progress";

describe("player progress", () => {
  it("advances only while playing and clamps to duration", () => {
    const snapshot = createDemoSnapshot("1");
    const anchor = snapshot.track?.anchorUnixMs ?? 0;
    expect(interpolatedPositionMs(snapshot, anchor + 2_000)).toBe(106_000);
    expect(interpolatedPositionMs(snapshot, anchor + 999_000)).toBe(256_000);
    expect(
      interpolatedPositionMs({ ...snapshot, state: "paused" }, anchor + 2_000)
    ).toBe(104_000);
  });

  it("mirrors the one-minute sample-clocked horizontal orbit", () => {
    const snapshot = createDemoSnapshot("1");
    const anchor = snapshot.track?.anchorUnixMs ?? 0;
    const frontOrigin = { ...snapshot, orbitDegrees: 0 };
    expect(orbitDegreesAt(frontOrigin, anchor)).toBe(264);
    expect(orbitDegreesAt(frontOrigin, anchor + 16_000)).toBe(0);
    expect(orbitDegreesAt({ ...frontOrigin, state: "paused" }, anchor + 16_000)).toBe(264);
  });

  it("formats durations and clamps percentages", () => {
    expect(formatDuration(65_000)).toBe("1:05");
    expect(formatDuration(3_665_000)).toBe("1:01:05");
    expect(formatDuration(null)).toBe("LIVE");
    expect(progressPercent(200, 100)).toBe(100);
    expect(progressPercent(-1, 100)).toBe(0);
    expect(progressPercent(50, null)).toBe(0);
  });
});
