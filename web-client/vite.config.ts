import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

// The dashboard is served by the bot at the same origin in production. In dev,
// run `vite` here (port 5173) and the bot's web server on :8080, and set
// WEB_PUBLIC_URL/OAUTH_REDIRECT_URI to the :5173 origin so the proxied /api and
// /auth calls (and the OAuth round-trip) stay same-origin. See the README.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  build: {
    // Output straight into the bot's dist/ so routes/static.ts can serve it.
    outDir: '../dist/web-client',
    emptyOutDir: true,
  },
  server: {
    port: 5173,
    proxy: {
      '/api': { target: 'http://localhost:8080', changeOrigin: false },
      '/auth': { target: 'http://localhost:8080', changeOrigin: false },
    },
  },
});
