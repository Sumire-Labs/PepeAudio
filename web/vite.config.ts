import react from "@vitejs/plugin-react";
import stylex from "@stylexjs/unplugin";
import { defineConfig } from "vitest/config";
import { licenseAssets } from "./build-license-assets.ts";

export default defineConfig(({ mode }) => ({
  plugins: [
    mode !== "test" && stylex.vite({ useCSSLayers: true }),
    react(),
    licenseAssets()
  ],
  server: {
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
    proxy: {
      "/api": "http://127.0.0.1:3000",
      "/auth": "http://127.0.0.1:3000"
    }
  },
  build: {
    target: "es2023",
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [
            {
              name: "astryx",
              test: /node_modules\/(?:@astryxdesign|@stylexjs|@formatjs|intl-messageformat|css-mediaquery|styleq|invariant)\//u
            },
            {
              name: "react",
              test: /node_modules\/(?:react|react-dom|scheduler)\//u
            },
            {
              name: "icons",
              test: /node_modules\/lucide-react\//u
            }
          ]
        }
      }
    }
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    restoreMocks: true
  }
}));
