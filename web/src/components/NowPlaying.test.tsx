import { cleanup, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { createDemoSnapshot, demoGuilds } from "../app/demo-data";
import type { PlayerSnapshot } from "../app/types";
import { NowPlaying } from "./NowPlaying";

const { sectionRender } = vi.hoisted(() => ({ sectionRender: vi.fn() }));

vi.mock("@astryxdesign/core/Section", () => ({
  Section: ({ children }: { readonly children?: ReactNode }) => {
    sectionRender();
    return <section>{children}</section>;
  }
}));

afterEach(() => {
  cleanup();
  sectionRender.mockClear();
});

describe("NowPlaying connection state", () => {
  it("does not rerender when only the player clock changes outside its stable props", () => {
    const snapshot = createDemoSnapshot("guild-1");
    const guild = demoGuilds[0];
    const { rerender } = render(
      <NowPlaying guild={guild} snapshot={snapshot} />
    );
    const initialRenders = sectionRender.mock.calls.length;
    expect(initialRenders).toBeGreaterThan(0);

    rerender(
      <NowPlaying guild={guild} snapshot={snapshot} />
    );

    expect(sectionRender).toHaveBeenCalledTimes(initialRenders);
  });

  it("does not claim that voice or 360° Audio is active while disconnected", () => {
    const snapshot: PlayerSnapshot = {
      ...createDemoSnapshot("guild-1"),
      state: "disconnected",
      voiceConnected: false,
      voiceChannelName: null,
      spatialEnabled: false
    };

    render(<NowPlaying guild={demoGuilds[0]} snapshot={snapshot} />);

    expect(screen.getAllByText("未接続")).toHaveLength(2);
    expect(screen.getByText("オフ")).toBeTruthy();
    expect(screen.queryByText("Connected")).toBeNull();
    expect(screen.queryByText("HRIR適用中")).toBeNull();
  });

  it("only shows the artwork marker when 360° Audio is enabled", () => {
    const snapshot = createDemoSnapshot("guild-1");
    const { rerender } = render(
      <NowPlaying guild={demoGuilds[0]} snapshot={snapshot} />
    );

    expect(screen.getByText("360°")).toBeTruthy();
    rerender(
      <NowPlaying
        guild={demoGuilds[0]}
        snapshot={{ ...snapshot, spatialEnabled: false }}
      />
    );
    expect(screen.queryByText("360°")).toBeNull();
  });

  it("uses a generic voice label and omits unavailable live metadata", () => {
    const fixture = createDemoSnapshot("guild-1");
    if (fixture.track === null) throw new Error("demo track fixture is missing");
    const snapshot: PlayerSnapshot = {
      ...fixture,
      voiceConnected: true,
      voiceChannelName: null,
      track: {
        ...fixture.track,
        artist: null,
        album: null,
        requestedBy: null
      }
    };

    render(<NowPlaying guild={demoGuilds[0]} snapshot={snapshot} />);

    expect(screen.getByText("接続中")).toBeTruthy();
    expect(screen.queryByText("Discord audio")).toBeNull();
    expect(screen.queryByText("PepeAudio queue")).toBeNull();
    expect(screen.queryByText(/リクエスト/u)).toBeNull();
  });

  it("shows the validated catalog origin for the current track", () => {
    const fixture = createDemoSnapshot("guild-1");
    if (fixture.track === null) throw new Error("demo track fixture is missing");
    render(
      <NowPlaying
        guild={demoGuilds[0]}
        snapshot={{
          ...fixture,
          track: {
            ...fixture.track,
            provenance: {
              origin: {
                provider: "spotify",
                url: "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC"
              },
              playback: {
                provider: "youtube",
                url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
              }
            }
          }
        }}
      />
    );

    expect(screen.getByRole("link", { name: /Spotifyで開く/u })).toBeTruthy();
  });

  it("does not describe a paused track as playing", () => {
    const snapshot = { ...createDemoSnapshot("guild-1"), state: "paused" as const };
    render(<NowPlaying guild={demoGuilds[0]} snapshot={snapshot} />);

    expect(screen.getAllByText("一時停止中")).toHaveLength(2);
    expect(screen.queryByText("Discordで再生中")).toBeNull();
  });

  it("distinguishes a trackless loading state from an empty player", () => {
    const snapshot: PlayerSnapshot = {
      ...createDemoSnapshot("guild-1"),
      state: "loading",
      track: null
    };
    const { container } = render(
      <NowPlaying guild={demoGuilds[0]} snapshot={snapshot} />
    );

    expect(screen.getByText("次の曲を読み込んでいます")).toBeTruthy();
    expect(screen.getByRole("status")).toBeTruthy();
    expect(container.querySelector(".astryx-spinner")).toBeTruthy();
    expect(screen.queryByText("再生待ちです")).toBeNull();
  });

  it("uses cards for playback metrics instead of nested full-bleed sections", () => {
    const snapshot: PlayerSnapshot = {
      ...createDemoSnapshot("guild-1"),
      state: "disconnected",
      voiceConnected: false,
      track: null,
      spatialEnabled: false
    };

    render(<NowPlaying guild={demoGuilds[0]} snapshot={snapshot} />);

    const cards = ["再生", "ボイス", "360° Audio"].map((label) =>
      screen.getByText(label).closest(".astryx-card")
    );
    expect(cards.every(Boolean)).toBe(true);
    expect(new Set(cards).size).toBe(3);
    expect(sectionRender).toHaveBeenCalledTimes(1);
  });
});
