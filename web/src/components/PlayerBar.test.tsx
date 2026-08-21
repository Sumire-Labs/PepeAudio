import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { createDemoSnapshot, demoGuilds, demoPresets } from "../app/demo-data";
import type { DashboardModel } from "../app/types";
import { PlayerBar } from "./PlayerBar";

afterEach(cleanup);

function model(
  setVolume: DashboardModel["setVolume"],
  seek: DashboardModel["seek"] = vi.fn()
): DashboardModel {
  const snapshot = createDemoSnapshot(demoGuilds[0]?.id ?? "1");
  return {
    guilds: demoGuilds,
    selectedGuildId: snapshot.guildId,
    snapshot,
    presets: demoPresets,
    hrirCatalogStatus: "ready",
    connected: true,
    commandPending: false,
    selectGuild: vi.fn(),
    enqueueMedia: vi.fn(),
    togglePlayback: vi.fn(),
    skip: vi.fn(),
    previous: vi.fn(),
    stop: vi.fn(),
    removeQueued: vi.fn(),
    moveQueued: vi.fn(),
    toggleShuffle: vi.fn(),
    cycleRepeat: vi.fn(),
    setVolume,
    setPreset: vi.fn(),
    toggleSpatial: vi.fn(),
    seek
  };
}

describe("PlayerBar volume commit", () => {
  it("routes transport and playback-mode controls without changing their identities", () => {
    const dashboard = model(vi.fn());
    render(<PlayerBar model={dashboard} />);

    fireEvent.click(screen.getByRole("button", { name: "一時停止" }));
    fireEvent.click(screen.getByRole("button", { name: "前の曲" }));
    fireEvent.click(screen.getByRole("button", { name: "次の曲" }));
    fireEvent.click(screen.getByRole("button", { name: "停止" }));
    fireEvent.click(screen.getByRole("button", { name: "シャッフル" }));
    fireEvent.click(screen.getByRole("button", {
      name: "リピート設定を切り替え（現在: オフ）"
    }));

    expect(dashboard.togglePlayback).toHaveBeenCalledOnce();
    expect(dashboard.previous).toHaveBeenCalledOnce();
    expect(dashboard.skip).toHaveBeenCalledOnce();
    expect(dashboard.stop).toHaveBeenCalledOnce();
    expect(dashboard.toggleShuffle).toHaveBeenCalledOnce();
    expect(dashboard.cycleRepeat).toHaveBeenCalledOnce();
  });

  it("does not submit an unchanged slider value", () => {
    const setVolume = vi.fn();
    render(<PlayerBar model={model(setVolume)} />);
    const slider = screen.getByRole("slider", { name: "音量" });

    fireEvent.pointerUp(slider.parentElement ?? slider);

    expect(setVolume).not.toHaveBeenCalled();
  });

  it("submits one value when the user commits a change", () => {
    const setVolume = vi.fn();
    render(<PlayerBar model={model(setVolume)} />);
    const slider = screen.getByRole("slider", { name: "音量" });

    fireEvent.keyDown(slider, { key: "ArrowLeft" });

    expect(setVolume).toHaveBeenCalledOnce();
    expect(setVolume).toHaveBeenCalledWith(70);
  });

  it("reconciles a volume draft when a command finishes without a new revision", () => {
    const setVolume = vi.fn();
    const dashboard = model(setVolume);
    const { rerender } = render(<PlayerBar model={dashboard} />);
    const slider = screen.getByRole("slider", { name: "音量" });

    fireEvent.keyDown(slider, { key: "ArrowLeft" });
    expect(slider.getAttribute("aria-valuenow")).toBe("70");

    rerender(
      <PlayerBar model={{ ...dashboard, commandPending: true }} />
    );
    expect(screen.getByRole("slider", { name: "音量" }).getAttribute("aria-valuenow"))
      .toBe("70");
    rerender(<PlayerBar model={dashboard} />);

    expect(screen.getByRole("slider", { name: "音量" }).getAttribute("aria-valuenow"))
      .toBe("75");
  });

  it("reconciles a seek draft when a command finishes without a new revision", () => {
    const seek = vi.fn();
    const dashboard = model(vi.fn(), seek);
    const { rerender } = render(<PlayerBar model={dashboard} />);
    const slider = screen.getByRole("slider", { name: "再生位置" });
    const authoritative = slider.getAttribute("aria-valuenow");

    fireEvent.keyDown(slider, { key: "ArrowLeft" });
    expect(slider.getAttribute("aria-valuenow")).not.toBe(authoritative);

    rerender(
      <PlayerBar model={{ ...dashboard, commandPending: true }} />
    );
    expect(screen.getByRole("slider", { name: "再生位置" }).getAttribute("aria-valuenow"))
      .not.toBe(authoritative);
    rerender(<PlayerBar model={dashboard} />);

    expect(screen.getByRole("slider", { name: "再生位置" }).getAttribute("aria-valuenow"))
      .toBe(authoritative);
  });

  it("commits a touch-cancelled volume change only once", () => {
    const setVolume = vi.fn();
    render(<PlayerBar model={model(setVolume)} />);
    const slider = screen.getByRole("slider", { name: "音量" });

    const track = slider.parentElement;
    if (track === null) throw new Error("volume slider track is missing");
    vi.spyOn(track, "getBoundingClientRect").mockReturnValue({
      bottom: 20,
      height: 20,
      left: 0,
      right: 100,
      top: 0,
      width: 100,
      x: 0,
      y: 0,
      toJSON: () => ({})
    });
    fireEvent.pointerDown(track, { clientX: 38, clientY: 10, pointerId: 1 });
    fireEvent.pointerCancel(track, { pointerId: 1 });
    fireEvent.blur(slider);

    expect(setVolume).toHaveBeenCalledOnce();
    expect(setVolume).toHaveBeenCalledWith(40);
  });

  it("exposes a readable value for both ranges", () => {
    render(<PlayerBar model={model(vi.fn())} />);

    expect(
      screen.getByRole("slider", { name: "再生位置" }).getAttribute("aria-valuetext")
    ).toMatch(/^\d+:\d{2}$/);
    expect(
      screen.getByRole("slider", { name: "音量" }).getAttribute("aria-valuetext")
    ).toBe("75%");
  });

  it("commits keyboard seeking without resubmitting on blur", () => {
    const seek = vi.fn();
    const dashboard = model(vi.fn(), seek);
    render(<PlayerBar model={dashboard} />);
    const slider = screen.getByRole("slider", { name: "再生位置" });

    fireEvent.keyDown(slider, { key: "ArrowLeft" });
    const committedValue = Number(slider.getAttribute("aria-valuenow"));
    fireEvent.blur(slider);

    expect(seek).toHaveBeenCalledOnce();
    expect(seek).toHaveBeenCalledWith(committedValue);
  });

  it("blocks every mutation control until the player snapshot is synchronized", () => {
    const dashboard = { ...model(vi.fn()), connected: false };
    render(<PlayerBar model={dashboard} />);

    for (const name of [
      "シャッフル",
      "前の曲",
      "一時停止",
      "次の曲",
      "リピート設定を切り替え（現在: オフ）",
      "停止"
    ]) {
      expect(screen.getByRole("button", { name }).getAttribute("aria-disabled")).toBe("true");
    }
    expect(screen.getByRole("slider", { name: "音量" }).getAttribute("aria-disabled")).toBe("true");
    expect(screen.getByRole("slider", { name: "再生位置" }).getAttribute("aria-disabled")).toBe("true");
  });

  it("labels the trackless loading state as loading instead of waiting", () => {
    const dashboard = model(vi.fn());
    render(
      <PlayerBar
        model={{
          ...dashboard,
          snapshot: { ...dashboard.snapshot, state: "loading", track: null }
        }}
      />
    );

    expect(screen.getByText("読み込み中")).toBeTruthy();
    expect(screen.getByText("次の曲を準備しています")).toBeTruthy();
    expect(screen.queryByText("再生待ち")).toBeNull();
  });

  it("keeps sliders outside the Astryx transport toolbar", () => {
    render(<PlayerBar model={model(vi.fn())} />);

    const toolbar = screen.getByRole("toolbar", { name: "再生コントロール" });
    expect(within(toolbar).queryAllByRole("slider")).toHaveLength(0);
  });

  it("keeps keyboard focus on a range while changing its value", () => {
    render(<PlayerBar model={model(vi.fn())} />);
    const slider = screen.getByRole("slider", { name: "音量" });

    slider.focus();
    fireEvent.keyDown(slider, { key: "ArrowLeft" });

    expect(document.activeElement).toBe(slider);
  });

  it("uses a neutral placeholder and does not label trackless playback as live", () => {
    const dashboard = model(vi.fn());
    const { container } = render(
      <PlayerBar
        model={{
          ...dashboard,
          snapshot: {
            ...dashboard.snapshot,
            state: "idle_connected",
            track: null
          }
        }}
      />
    );

    const placeholder = screen.getByTestId("player-artwork");
    expect(placeholder.querySelector('.astryx-icon[data-color="secondary"]')).not.toBeNull();
    expect(screen.queryByText("LIVE")).toBeNull();
    expect(container.textContent?.match(/0:00/g)).toHaveLength(2);
  });

  it("shows live only for a real track without a finite duration", () => {
    const dashboard = model(vi.fn());
    const track = dashboard.snapshot.track;
    if (track === null) throw new Error("demo track is missing");

    render(
      <PlayerBar
        model={{
          ...dashboard,
          snapshot: {
            ...dashboard.snapshot,
            track: { ...track, durationMs: null, seekable: false }
          }
        }}
      />
    );

    expect(screen.getByText("LIVE")).toBeTruthy();
  });

  it("renders only safe HTTPS artwork URLs", () => {
    const dashboard = model(vi.fn());
    const track = dashboard.snapshot.track;
    if (track === null) throw new Error("demo track is missing");

    const { container, rerender } = render(
      <PlayerBar
        model={{
          ...dashboard,
          snapshot: {
            ...dashboard.snapshot,
            track: { ...track, artworkUrl: "https://cdn.example.test/cover.jpg" }
          }
        }}
      />
    );

    expect(container.querySelector("img")?.getAttribute("src"))
      .toBe("https://cdn.example.test/cover.jpg");

    rerender(
      <PlayerBar
        model={{
          ...dashboard,
          snapshot: {
            ...dashboard.snapshot,
            track: { ...track, artworkUrl: "javascript:alert(1)" }
          }
        }}
      />
    );

    expect(container.querySelector("img")).toBeNull();
  });

  it("uses the muted-volume icon at zero percent", () => {
    const dashboard = model(vi.fn());
    render(
      <PlayerBar
        model={{
          ...dashboard,
          snapshot: { ...dashboard.snapshot, volumePercent: 0 }
        }}
      />
    );

    expect(screen.getByTestId("player-volume-icon").classList)
      .toContain("lucide-volume-x");
  });
});
