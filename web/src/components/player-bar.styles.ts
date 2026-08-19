import {
  colorVars,
  radiusVars,
  spacingVars
} from "@astryxdesign/core/theme/tokens.stylex";
import * as stylex from "@stylexjs/stylex";

export const playerBarStyles = stylex.create({
  root: {
    backgroundColor: colorVars["--color-background-surface"]
  },
  playerGrid: {
    paddingBlockStart: spacingVars["--spacing-2"],
    paddingInline: spacingVars["--spacing-3"]
  },
  trackSummary: {
    minWidth: 0,
    width: "100%"
  },
  artwork: {
    backgroundColor: colorVars["--color-background-muted"],
    borderRadius: radiusVars["--radius-element"],
    width: spacingVars["--spacing-12"]
  },
  artworkPlaceholder: {
    backgroundColor: colorVars["--color-background-muted"]
  },
  transportToolbar: {
    width: "100%"
  },
  volumeZone: {
    minWidth: 0,
    width: "100%",
    "@media (max-width: 1024px)": {
      gridColumn: "1 / -1"
    }
  },
  rangeValue: {
    flexShrink: 0,
    whiteSpace: "nowrap"
  },
  seekRow: {
    minWidth: 0,
    paddingBlock: spacingVars["--spacing-2"],
    paddingInline: spacingVars["--spacing-3"],
    width: "100%"
  }
});
