import {
  colorVars,
  radiusVars
} from "@astryxdesign/core/theme/tokens.stylex";
import * as stylex from "@stylexjs/stylex";

export const queuePanelStyles = stylex.create({
  sortableItem: {
    position: "relative"
  },
  draggingItem: {
    backgroundColor: colorVars["--color-accent-muted"],
    borderRadius: radiusVars["--radius-element"],
    opacity: 0.82,
    zIndex: 1
  },
  dragHandle: {
    cursor: "grab",
    flexShrink: 0,
    touchAction: "none",
    ":active": {
      cursor: "grabbing"
    }
  }
});
