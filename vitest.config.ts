import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

// happy-dom is lighter than jsdom for Tauri-style code that doesn't exercise
// the full browser surface (no <iframe>, no <canvas> beyond stubs).
// Coverage / threads inherit vitest defaults; we explicitly disable file
// watching in CI by running `npm run test:run` (one-shot mode).
export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'happy-dom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    css: false,
  },
});
