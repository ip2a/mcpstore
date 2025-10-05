import { defineConfig, loadEnv } from 'vite'
import vue from '@vitejs/plugin-vue'
import path from 'path'
import viteCompression from 'vite-plugin-compression'
import Components from 'unplugin-vue-components/vite'
import AutoImport from 'unplugin-auto-import/vite'
import { ElementPlusResolver } from 'unplugin-vue-components/resolvers'
import { fileURLToPath } from 'url'
// import viteImagemin from 'vite-plugin-imagemin'
// import { visualizer } from 'rollup-plugin-visualizer'

// https://devtools.vuejs.org/getting-started/introduction
import vueDevTools from 'vite-plugin-vue-devtools'

export default ({ mode }: { mode: string }) => {
  const root = process.cwd()
  const env = loadEnv(mode, root)
  const { VITE_VERSION, VITE_PORT, VITE_BASE_URL, VITE_API_URL, VITE_API_PROXY_URL } = env

  // 生产环境默认配置
  const isProduction = mode === 'production'
  const defaultBaseUrl = '/web_demo/'  // 始终使用 /web_demo/ 作为基础路径
  const defaultApiUrl = '/api'  // 始终使用 /api 作为API路径
  const defaultApiProxyUrl = isProduction ? 'http://127.0.0.1:18200' : (VITE_API_PROXY_URL || 'http://localhost:18200')
  const defaultPort = 5177  // 始终使用5177端口

  console.log(`🚀 Mode = ${mode}`)
  console.log(`🚀 API_URL = ${defaultApiUrl}`)
  console.log(`🚀 API_PROXY_URL = ${defaultApiProxyUrl}`)
  console.log(`🚀 BASE_URL = ${defaultBaseUrl}`)
  console.log(`🚀 PORT = ${defaultPort}`)

  const baseForDev = '/'

  return defineConfig({
    define: {
      __APP_VERSION__: JSON.stringify(VITE_VERSION || '1.0.0')
    },
    base: isProduction ? (VITE_BASE_URL || defaultBaseUrl) : baseForDev,
    server: {
      port: defaultPort,
      host: '0.0.0.0', // 允许外部访问
      strictPort: true, // 如果端口被占用，直接失败而不是尝试其他端口
      allowedHosts: [
        'localhost',
        '127.0.0.1',
        'mcpstore.wiki',
        '.mcpstore.wiki' // 允许子域名
      ],
      cors: true, // 启用CORS
      proxy: {
        '/api': {
          target: defaultApiProxyUrl,
          changeOrigin: true,
          secure: false, // 本地开发不需要HTTPS
          rewrite: (path) => path.replace(/^\/api/, ''),
          configure: (proxy, options) => {
            proxy.on('error', (err, req, res) => {
              console.log('proxy error', err);
            });
            proxy.on('proxyReq', (proxyReq, req, res) => {
              console.log('Sending Request to the Target:', req.method, req.url);
            });
            proxy.on('proxyRes', (proxyRes, req, res) => {
              console.log('Received Response from the Target:', proxyRes.statusCode, req.url);
            });
          }
        }
      }
    },
    // 路径别名
    resolve: {
      alias: {
        '@': fileURLToPath(new URL('./src', import.meta.url)),
        '@styles': resolvePath('src/assets/styles'),
        // 以下别名为旧框架遗留，MCP-only 模式中计划删除；为兼容暂时保留
        '@views': resolvePath('src/views'),
        '@imgs': resolvePath('src/assets/img'),
        '@icons': resolvePath('src/assets/icons'),
        '@utils': resolvePath('src/utils'),
        '@stores': resolvePath('src/store'),
        '@plugins': resolvePath('src/plugins')
      }
    },
    build: {
      target: 'es2015',
      outDir: 'dist',
      chunkSizeWarningLimit: 2000,
      minify: 'terser',
      terserOptions: {
        compress: {
          drop_console: true, // 生产环境去除 console
          drop_debugger: true // 生产环境去除 debugger
        }
      },
      rollupOptions: {
        output: {
          manualChunks: {
            vendor: ['vue', 'vue-router', 'pinia', 'element-plus']
          }
        }
      },
      dynamicImportVarsOptions: {
        warnOnError: true,
        exclude: [],
        // MCP-only:    views   mcp 
        include: ['src/mcp/**/*.vue']
      }
    },
    plugins: [
      vue(),
      // 自动导入 components 下面的组件，无需 import 引入
      Components({
        deep: true,
        extensions: ['vue'],
        dirs: ['src/components'], // 自动导入的组件目录
        resolvers: [ElementPlusResolver()],
        dts: 'src/types/components.d.ts' // 指定类型声明文件的路径
      }),
      AutoImport({
        imports: ['vue', 'vue-router', '@vueuse/core', 'pinia'],
        resolvers: [ElementPlusResolver()],
        dts: 'src/types/auto-imports.d.ts',
        eslintrc: {
          // 这里先设置成true然后pnpm dev 运行之后会生成 .auto-import.json 文件之后，在改为false
          enabled: true,
          filepath: './.auto-import.json',
          globalsPropValue: true
        }
      }),
      // 打包分析
      // visualizer({
      //   open: true,
      //   gzipSize: true,
      //   brotliSize: true,
      //   filename: 'dist/stats.html' // 分析图生成的文件名及路径
      // }),
      // 压缩
      viteCompression({
        verbose: true, // 是否在控制台输出压缩结果
        disable: false, // 是否禁用
        algorithm: 'gzip', // 压缩算法,可选 [ 'gzip' , 'brotliCompress' ,'deflate' , 'deflateRaw']
        ext: '.gz', // 压缩后的文件名后缀
        threshold: 10240, // 只有大小大于该值的资源会被处理 10240B = 10KB
        deleteOriginFile: false // 压缩后是否删除原文件
      }),
      // 图片压缩
      // viteImagemin({
      //   verbose: true, // 是否在控制台输出压缩结果
      //   // 图片压缩配置
      //   // GIF 图片压缩配置
      //   gifsicle: {
      //     optimizationLevel: 4, // 优化级别 1-7，7为最高级别压缩
      //     interlaced: false // 是否隔行扫描
      //   },
      //   // PNG 图片压缩配置
      //   optipng: {
      //     optimizationLevel: 4 // 优化级别 0-7，7为最高级别压缩
      //   },
      //   // JPEG 图片压缩配置
      //   mozjpeg: {
      //     quality: 60 // 压缩质量 0-100，值越小压缩率越高
      //   },
      //   // PNG 图片压缩配置(另一个压缩器)
      //   pngquant: {
      //     quality: [0.8, 0.9], // 压缩质量范围 0-1
      //     speed: 4 // 压缩速度 1-11，值越大压缩速度越快，但质量可能会下降
      //   },
      //   // SVG 图片压缩配置
      //   svgo: {
      //     plugins: [
      //       {
      //         name: 'removeViewBox' // 移除 viewBox 属性
      //       },
      //       {
      //         name: 'removeEmptyAttrs', // 移除空属性
      //         active: false // 是否启用此插件
      //       }
      //     ]
      //   }
      // })
      vueDevTools()
    ],
    // 预加载项目必需的组件
    optimizeDeps: {
      include: [
        'vue',
        'vue-router',
        'pinia',
        'axios',
        '@vueuse/core',
        'echarts',
        'element-plus',
        'vue-i18n'
        // MCP-only: 精简预打包依赖，移除大量旧框架/演示依赖（如 wangeditor、xlsx、file-saver、vue-img-cutter 以及 Element Plus 样式细分条目）。
        // 如遇到首次启动预打包变慢，可按需补回。
      ]
    },
    css: {
      preprocessorOptions: {
        // sass variable and mixin
        scss: {
          api: 'modern-compiler',
          additionalData: `
            @use "@styles/variables.scss" as *; @use "@styles/mixin.scss" as *;
          `
        }
      },
      postcss: {
        plugins: [
          {
            postcssPlugin: 'internal:charset-removal',
            AtRule: {
              charset: (atRule) => {
                if (atRule.name === 'charset') {
                  atRule.remove()
                }
              }
            }
          }
        ]
      }
    }
  })
}

function resolvePath(paths: string) {
  return path.resolve(__dirname, paths)
}
