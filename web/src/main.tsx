import { InternationalizationProvider } from "@astryxdesign/core/i18n";
import { Theme } from "@astryxdesign/core/theme";
import { ToastViewport } from "@astryxdesign/core/Toast";
import { neutralTheme } from "@astryxdesign/theme-neutral/built";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./app/App";
import { astryxJapaneseMessages } from "./app/astryx-ja";
import "@astryxdesign/core/reset.css";
import "@astryxdesign/core/astryx.css";
import "@astryxdesign/theme-neutral/theme.css";

const root = document.getElementById("root");
if (root === null) {
  throw new Error("PepeAudio root element is missing");
}

createRoot(root).render(
  <StrictMode>
    <Theme theme={neutralTheme} mode="system">
      <InternationalizationProvider
        locale="ja"
        messages={{ ja: astryxJapaneseMessages }}
      >
        <ToastViewport position="bottomEnd" maxVisible={3}>
          <App />
        </ToastViewport>
      </InternationalizationProvider>
    </Theme>
  </StrictMode>
);
