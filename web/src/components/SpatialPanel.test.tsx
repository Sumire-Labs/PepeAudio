import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { createDemoSnapshot } from "../app/demo-data";
import { SpatialPanel } from "./SpatialPanel";

afterEach(cleanup);

const baseProps = {
  selectedPresetId: null,
  snapshot: {
    ...createDemoSnapshot("guild-1"),
    state: "paused" as const,
    spatialEnabled: false,
    orbitDegrees: 42,
    track: null
  },
  connected: true,
  commandPending: false,
  onPresetChange: vi.fn(),
  onToggle: vi.fn()
} as const;

describe("SpatialPanel catalog states", () => {
  it("reads spatial state and direction from the player snapshot", () => {
    render(<SpatialPanel {...baseProps} presets={[]} catalogStatus="ready" />);

    expect(screen.getByText("空間処理はオフ")).toBeTruthy();
    expect(screen.getByText("42°")).toBeTruthy();
    expect(screen.getByLabelText("現在の水平音源方向")).toBeTruthy();
  });

  it("distinguishes an unavailable catalog from an empty catalog", () => {
    const { rerender } = render(
      <SpatialPanel {...baseProps} presets={[]} catalogStatus="unavailable" />
    );
    expect(screen.getByText("HRIRカタログを取得できませんでした。")).toBeTruthy();
    expect(screen.getByRole("combobox").getAttribute("aria-disabled")).toBe("true");

    rerender(<SpatialPanel {...baseProps} presets={[]} catalogStatus="ready" />);
    expect(
      screen.getByText("このサーバーで利用可能なプリセットはありません。")
    ).toBeTruthy();
  });

  it("shows public attribution and a safe source link for the selected preset", () => {
    render(
      <SpatialPanel
        {...baseProps}
        catalogStatus="ready"
        selectedPresetId="neutral"
        presets={[{
          id: "neutral",
          name: "Neutral",
          source: {
            licenseName: "CC0-1.0",
            sourceUrl: "https://example.test/source",
            attribution: "Fixture author"
          }
        }]}
      />
    );

    expect(screen.getByText("Fixture author · CC0-1.0")).toBeTruthy();
    expect(screen.getByRole("link").getAttribute("href")).toBe(
      "https://example.test/source"
    );
  });

  it("uses the Astryx switch for spatial audio and blocks duplicate commands", () => {
    const onToggle = vi.fn();
    const { rerender } = render(
      <SpatialPanel
        {...baseProps}
        catalogStatus="ready"
        presets={[]}
        onToggle={onToggle}
      />
    );

    fireEvent.click(screen.getByRole("switch", { name: "360° Audio" }));
    expect(onToggle).toHaveBeenCalledOnce();

    onToggle.mockClear();
    rerender(
      <SpatialPanel
        {...baseProps}
        catalogStatus="ready"
        presets={[]}
        commandPending
        onToggle={onToggle}
      />
    );
    fireEvent.click(screen.getByRole("switch", { name: "360° Audio" }));
    expect(onToggle).not.toHaveBeenCalled();
  });

  it("disables the switch and preset selector while the player is unsynchronized", () => {
    const onToggle = vi.fn();
    const onPresetChange = vi.fn();
    render(
      <SpatialPanel
        {...baseProps}
        connected={false}
        catalogStatus="ready"
        presets={[{
          id: "neutral",
          name: "Neutral",
          source: { licenseName: null, sourceUrl: null, attribution: null }
        }]}
        onToggle={onToggle}
        onPresetChange={onPresetChange}
      />
    );

    const spatialSwitch = screen.getByRole("switch", { name: "360° Audio" });
    const selector = screen.getByRole("combobox", { name: "HRIRプリセット" });
    expect(spatialSwitch.getAttribute("aria-disabled")).toBe("true");
    expect(selector.getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(spatialSwitch);
    fireEvent.click(selector);
    expect(onToggle).not.toHaveBeenCalled();
    expect(onPresetChange).not.toHaveBeenCalled();
  });
});
