import { readFileSync } from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"
import react from "@vitejs/plugin-react"
import tailwindcss from "@tailwindcss/vite"
import { defineConfig } from "vite"

const dirname = path.dirname(fileURLToPath(import.meta.url))
const webPackageVersion = JSON.parse(readFileSync(path.join(dirname, "package.json"), "utf8")).version as string

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
    port: 1828,
    // App API 代理保留（本地控制面）
    proxy: {
      "/api": {
        target: "http://127.0.0.1:1821",
        changeOrigin: true,
        rewrite: (route) => route.replace(/^\/api/, ""),
      },
    },
  },
})
