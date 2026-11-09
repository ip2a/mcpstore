import { readFileSync } from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"
import react from "@vitejs/plugin-react"
import tailwindcss from "@tailwindcss/vite"
import { defineConfig } from "vite"

const dirname = path.dirname(fileURLToPath(import.meta.url))
const webPackageVersion = JSON.parse(readFileSync(path.join(dirname, "package.json"), "utf8")).version as string

// The /api owner is the standalone panel process (`mcpstore web`); override for
// isolated dev runs (run_debug.sh sets this when MCPSTORE_PANEL_PORT moves).
const panelTarget = process.env.MCPSTORE_PANEL_TARGET ?? "http://127.0.0.1:1828"

export default defineConfig({
  plugins: [react(), tailwindcss()],
  define: {
    __WEB_APP_VERSION__: JSON.stringify(webPackageVersion),
  },
  // Isolate the dep pre-bundle cache per checkout when node_modules is a shared symlink,
  // so lockfile churn on one checkout doesn't force re-optimization in the other
  cacheDir: path.resolve(dirname, "../local/vite-cache"),
  resolve: {
    alias: {
      "@": path.resolve(dirname, "./src"),
    },
  },
  server: {
    // Keep the dev port away from the panel's 1828: vite takes 1829, /api proxies as-is to the panel process
    port: 1829,
    proxy: {
      "/api": {
        target: panelTarget,
        changeOrigin: true,
      },
    },
  },
})
