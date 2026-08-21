import type { ResizableProps } from "@astryxdesign/core/Resizable";

export const INSPECTOR_RAIL_WIDTH = 64;

interface InspectorPanelSizing {
  readonly hasDivider: boolean;
  readonly resizable?: ResizableProps;
  readonly width?: number;
}

export function inspectorPanelSizing(
  isCollapsed: boolean,
  resizable: ResizableProps
): InspectorPanelSizing {
  return isCollapsed
    ? { hasDivider: true, width: INSPECTOR_RAIL_WIDTH }
    : { hasDivider: false, resizable };
}
