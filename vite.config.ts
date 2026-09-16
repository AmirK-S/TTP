import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// Bake package.json version into the bundle so the JS Sentry SDK can tag
// events with the same release tag as the Rust SDK (sentry::release_name!()
// uses Cargo.toml version, kept in lockstep with package.json).
const pkgVersion = JSON.parse(
  readFileSync(fileURLToPath(new URL("./package.json", import.meta.url)), "utf-8")
).version as string;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],

  define: {
    "import.meta.env.VITE_APP_VERSION": JSON.stringify(pkgVersion),
  },

  build: {
    // 'hidden' emits .map files alongside the JS but does not embed a
    // sourceMappingURL comment in the bundle — devtools won't auto-load them
    // for end-users, but CI uploads them to Sentry so server-side symbolication
    // resolves minified JS panics to real function names. Without this, every
    // @sentry/react event arrives as obfuscated frames and triage is blind.
    sourcemap: "hidden",
    rollupOptions: {
      output: {
        manualChunks: {
          tauri: ["@tauri-apps/api"],
          lucide: ["lucide-react"],
          // Force @sentry/react into its own chunk so that:
          //  1. It only loads when initSentryIfConsented() resolves the dynamic
          //     import (i.e. telemetry-opted-in Settings/Onboarding windows).
          //  2. The pill window's static bundle stays SDK-free.
          //  3. Repeat visits to telemetry-bearing windows hit the same cached chunk.
          sentry: ["@sentry/react"],
        },
      },
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
