import { spacingVars } from "@astryxdesign/core/theme/tokens.stylex";
import * as stylex from "@stylexjs/stylex";

export const guildSidebarStyles = stylex.create({
  root: {
    minWidth: `calc(${spacingVars["--spacing-12"]} + ${spacingVars["--spacing-5"]})`
  }
});
