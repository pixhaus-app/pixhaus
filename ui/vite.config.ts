import { defineConfig, type UserConfig } from "vite";
import solid from "vite-plugin-solid";

const host = process.env.TAURI_DEV_HOST;

const server: UserConfig["server"] = {
  port: 1420,
  strictPort: true,
  host: host ?? false,
  watch: {
    ignored: ["**/src-tauri/**", "**/target/**"],
  },
};

if (host) {
  server.hmr = {
    protocol: "ws",
    host,
    port: 1421,
  };
}

export default defineConfig({
  plugins: [solid()],
  clearScreen: false,
  server,
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    // Vite 8 defaults to "baseline-widely-available" which is well below the
    // Tauri 2 webview floor (WebView2 on Windows, WebKit on macOS 13+,
    // WebKitGTK on Linux). Keep explicit per-platform targets above that
    // baseline so esbuild's transpiler doesn't strip features the webview
    // already supports.
    target: process.env.TAURI_PLATFORM === "windows" ? "chrome120" : "safari16",
    minify: process.env.TAURI_DEBUG ? false : "esbuild",
    sourcemap: Boolean(process.env.TAURI_DEBUG),
  },
});
