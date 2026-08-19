import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { createDemoSnapshot } from "../app/demo-data";
import { DashboardInspector } from "./DashboardInspector";

afterEach(cleanup);

const baseProps = {
  queue: [],
  presets: [],
  catalogStatus: "ready",
  selectedPresetId: null,
  snapshot: {
    ...createDemoSnapshot("guild-1"),
    spatialEnabled: false
  },
  connected: false,
  commandPending: false,
  onRemove: vi.fn(),
  onMove: vi.fn(),
  onPresetChange: vi.fn(),
  onSpatialToggle: vi.fn()
} as const;

describe("DashboardInspector presentation", () => {
  it("gives the queue the fill-sized scroll region in the side panel", () => {
    const { container } = render(
      <DashboardInspector {...baseProps} presentation="panel" />
    );

    const queueSection = screen.getByRole("heading", { name: "次に再生" })
      .closest(".astryx-section");
    const spatialSection = screen.getByRole("heading", { name: "360° Audio" })
      .closest(".astryx-section");
    expect(queueSection).toBeTruthy();
    expect(spatialSection).toBeTruthy();
    expect(queueSection).not.toBe(spatialSection);
    expect(container.querySelectorAll(".astryx-section")).toHaveLength(2);
    const queueRegion = screen.getByRole("region", { name: "次に再生キュー" });
    expect(queueRegion.getAttribute("data-size")).toBe("fill");
    expect(queueRegion.contains(queueSection)).toBe(true);
    expect(spatialSection?.closest(".astryx-stack-item")?.getAttribute("data-size"))
      .toBe("static");
    expect(screen.queryByRole("toolbar")).toBeNull();
  });

  it("makes the queue a full-width primary card when the inspector joins the content", () => {
    render(<DashboardInspector {...baseProps} presentation="content" />);

    const queueCard = screen.getByRole("heading", { name: "次に再生" })
      .closest(".astryx-card");
    const spatialCard = screen.getByRole("heading", { name: "360° Audio" })
      .closest(".astryx-card");
    expect(queueCard).toBeTruthy();
    expect(spatialCard).toBeTruthy();
    expect(queueCard).not.toBe(spatialCard);
    expect(queueCard?.parentElement?.className).toContain("astryx-grid-span");
    expect(spatialCard?.parentElement?.className).toContain("astryx-grid-span");
    expect(spatialCard?.getAttribute("data-variant")).toBe("muted");
  });
});
