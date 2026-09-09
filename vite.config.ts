import { defineConfig } from 'vitest/config';

// Tauri sets TAURI_DEV_HOST when developing against a remote/mobile target.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    watch: {
      // The Rust side is rebuilt by `tauri dev`; Vite must not restart on it.
      ignored: ['**/src-tauri/**'],
    },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    // WebView2 on Windows 10/11 is Chromium-based and evergreen.
    target: 'chrome105',
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    outDir: 'dist',
  },
  test: {
    include: ['tests/**/*.test.ts'],
    environment: 'node',
  },
});
