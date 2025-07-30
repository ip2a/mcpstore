import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'path'
import AutoImport from 'unplugin-auto-import/vite'
import Components from 'unplugin-vue-components/vite'
import { ElementPlusResolver } from 'unplugin-vue-components/resolvers'

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => {
  // 两种环境配置
  const isDomain = mode === 'domain'
  const base = isDomain ? '/web_demo/' : '/'  // 域名模式需要正确的base路径

  return {
    plugins: [
      vue(),
      AutoImport({
        resolvers: [ElementPlusResolver()],
        imports: ['vue', 'vue-router', 'pinia'],
        dts: true
      }),
      Components({
        resolvers: [ElementPlusResolver()],
        dts: true
      })
    ],
    resolve: {
      alias: {
        '@': resolve(__dirname, 'src'),
        '@components': resolve(__dirname, 'src/components'),
        '@views': resolve(__dirname, 'src/views'),
        '@utils': resolve(__dirname, 'src/utils'),
        '@api': resolve(__dirname, 'src/api'),
        '@stores': resolve(__dirname, 'src/stores'),
        '@assets': resolve(__dirname, 'src/assets')
      }
    },
    base,
    server: {
      port: 5177,
      host: '0.0.0.0',  // 服务器监听所有接口，但客户端连接地址由HMR配置决定
      open: !isDomain,
      cors: true,

      // 根据环境配置HMR
      hmr: isDomain ? false : {  // 域名环境禁用HMR，避免复杂的代理问题
        port: 5177,
        host: 'localhost'
      },

      // 域名环境的额外配置
      ...(isDomain && {
        allowedHosts: ['mcpstore.wiki', 'localhost', '127.0.0.1', '0.0.0.0']
      })
    },
    build: {
      outDir: 'dist',
      assetsDir: 'assets',
      sourcemap: false,
      rollupOptions: {
        output: {
          chunkFileNames: 'js/[name]-[hash].js',
          entryFileNames: 'js/[name]-[hash].js',
          assetFileNames: 'assets/[name]-[hash].[ext]'
        }
      }
    },
    preview: {
      port: 5177,
      host: '0.0.0.0'
    },
    css: {
      preprocessorOptions: {
        scss: {
          additionalData: `@use "@/styles/variables.scss" as *;`
        }
      }
    }
  }
})
