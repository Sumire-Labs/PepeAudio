import type { ResizableProps } from "@astryxdesign/core/Resizable";
import { describe, expect, it, vi } from "vitest";

import {
  INSPECTOR_RAIL_WIDTH,
  inspectorPanelSizing
} from "./inspector-panel-sizing";

const resizable: ResizableProps = {
  _size: 440,
  _isCollapsed: false,
  _onResizeStart: vi.fn(),
  _onResizeMove: vi.fn(),
  _onResizeEnd: vi.fn(),
  _minSizePx: 360,
  _maxSizePx: 600,
  _snaps: [400, 440, 520],
  _collapsedSize: INSPECTOR_RAIL_WIDTH,
  _collapsible: true,
  _isResizableProps: true
};

describe("inspector panel sizing", () => {
  it("keeps a visible fixed-width rail when the resizable region is collapsed", () => {
    expect(inspectorPanelSizing(true, resizable)).toEqual({
      hasDivider: true,
      width: INSPECTOR_RAIL_WIDTH
    });
  });

  it("restores the resizable panel after expansion", () => {
    expect(inspectorPanelSizing(false, resizable)).toEqual({
      hasDivider: false,
      resizable
    });
  });
});
