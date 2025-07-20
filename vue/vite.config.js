import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'path'
import AutoImport from 'unplugin-auto-import/vite'
import Components from 'unplugin-vue-components/vite'
import { ElementPlusResolver } from 'unplugin-vue-components/resolvers'

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [
    vue(),
    AutoImport({
      resolvers: [ElementPlusResolver()],
      imports: [
        'vue',
        'vue-router',
        'pinia'
      ],
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
  // 方案2：开发环境使用根路径，通过nginx重写路径
  base: '/',
  server: {
    port: 5177,
    host: '0.0.0.0',
    open: false, // 通过域名访问，不自动打开本地浏览器
    cors: true,
    // 允许通过域名访问 - 方案1：指定允许的主机
    allowedHosts: [
      'mcpstore.wiki',
      'localhost',
      '127.0.0.1',
      '0.0.0.0'
    ],
    // HMR配置 - 通过域名进行热更新
    hmr: {
      port: 5177,
      host: 'mcpstore.wiki'
    },
    // 开发环境通过FRP+Nginx访问，不需要本地代理
    // API请求会通过 mcpstore.wiki/api/ 访问
  },
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
    sourcemap: false,
    minify: 'terser',
    // 确保构建后的资源路径正确
    rollupOptions: {
      output: {
        // 代码分割优化
        manualChunks: {
          vendor: ['vue', 'vue-router', 'pinia'],
          elementPlus: ['element-plus'],
          echarts: ['echarts', 'vue-echarts'],
        },
        // 确保资源文件名包含hash以避免缓存问题
        chunkFileNames: 'js/[name]-[hash].js',
        entryFileNames: 'js/[name]-[hash].js',
        assetFileNames: 'assets/[name]-[hash].[ext]'
      }
    }
  },
  // 预览模式配置（用于生产环境测试）
  preview: {
    port: 5177,
    host: '0.0.0.0',
    // 预览模式也需要配置基础路径
    base: '/web_demo/'
  },
  css: {
    preprocessorOptions: {
      scss: {
        additionalData: `@use "@/styles/variables.scss" as *;`
      }
    }
  }
})
