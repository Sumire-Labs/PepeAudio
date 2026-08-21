import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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

describe("NowPlaying music presentation", () => {
  it("does not rerender when stable props are reused", () => {
    const snapshot = createDemoSnapshot("guild-1");
    const guild = demoGuilds[0];
    const { rerender } = render(<NowPlaying guild={guild} snapshot={snapshot} />);
    const initialRenders = sectionRender.mock.calls.length;

    rerender(<NowPlaying guild={guild} snapshot={snapshot} />);

    expect(sectionRender).toHaveBeenCalledTimes(initialRenders);
  });

  it("removes the redundant playback, voice, and HRIR metric cards", () => {
    render(
      <NowPlaying
        guild={demoGuilds[0]}
        snapshot={{ ...createDemoSnapshot("guild-1"), state: "disconnected" }}
      />
    );

    expect(screen.queryByText("ボイス")).toBeNull();
    expect(screen.queryByText("HRIR適用中")).toBeNull();
    expect(screen.queryByText("360° Audio")).toBeNull();
    expect(sectionRender).toHaveBeenCalledTimes(1);
  });

  it("uses the track title itself as the validated external link", () => {
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

    const link = screen.getByRole("link", { name: /Signals After Rain/u });
    expect(link.getAttribute("href"))
      .toBe("https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC");
    expect(screen.queryByText(/Spotifyで開く/u)).toBeNull();
    expect(screen.queryByText(/YouTubeで再生/u)).toBeNull();
  });

  it("renders a safe artwork and falls back after an image failure", () => {
    const fixture = createDemoSnapshot("guild-1");
    if (fixture.track === null) throw new Error("demo track fixture is missing");
    const { container } = render(
      <NowPlaying
        guild={demoGuilds[0]}
        snapshot={{
          ...fixture,
          track: {
            ...fixture.track,
            artworkUrl: "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg"
          }
        }}
      />
    );

    const image = screen.getByRole("img", { name: /Signals After Rainのアートワーク/u });
    fireEvent.error(image);
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector(".lucide-audio-lines")).toBeTruthy();
  });

  it("distinguishes track loading from the empty player", () => {
    const loading: PlayerSnapshot = {
      ...createDemoSnapshot("guild-1"),
      state: "loading",
      track: null
    };
    const { rerender } = render(
      <NowPlaying guild={demoGuilds[0]} snapshot={loading} />
    );

    expect(screen.getByText("曲を準備しています")).toBeTruthy();
    rerender(
      <NowPlaying
        guild={demoGuilds[0]}
        snapshot={{ ...loading, state: "idle_connected" }}
      />
    );
    expect(screen.getByText("まだ何も再生していません")).toBeTruthy();
  });

  it("does not describe a paused track as playing", () => {
    render(
      <NowPlaying
        guild={demoGuilds[0]}
        snapshot={{ ...createDemoSnapshot("guild-1"), state: "paused" }}
      />
    );

    expect(screen.getAllByText("一時停止中")).toHaveLength(1);
    expect(screen.queryByText("Discordで再生中")).toBeNull();
  });
});
