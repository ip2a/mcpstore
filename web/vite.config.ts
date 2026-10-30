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
  // node_modules 是软链共享时，依赖预构建缓存按 checkout 隔离，避免 lockfile 来回失效互相重打包
  cacheDir: path.resolve(dirname, "../local/vite-cache"),
  resolve: {
    alias: {
      "@": path.resolve(dirname, "./src"),
    },
  },
  server: {
    // dev 端口避开面板 1828：vite 占 1829，/api 原样代理到面板进程
    port: 1829,
    proxy: {
      "/api": {
        target: panelTarget,
        changeOrigin: true,
      },
    },
  },
})
