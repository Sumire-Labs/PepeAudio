import {
  colorVars,
  radiusVars
} from "@astryxdesign/core/theme/tokens.stylex";
import * as stylex from "@stylexjs/stylex";

export const nowPlayingStyles = stylex.create({
  root: {
    minWidth: 0
  },
  artworkFrame: {
    width: "100%",
    borderRadius: radiusVars["--radius-page"],
    overflow: "hidden"
  },
  artworkSurface: {
    width: "100%",
    height: "100%",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    backgroundImage: `radial-gradient(circle, ${colorVars["--color-accent-muted"]}, ${colorVars["--color-background-card"]})`
  },
  trackDetails: {
    minWidth: 0
  }
});
