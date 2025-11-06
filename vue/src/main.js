import { createApp } from 'vue'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'
import * as ElementPlusIconsVue from '@element-plus/icons-vue'
import zhCn from 'element-plus/es/locale/lang/zh-cn'
import App from './App.vue'
import router from './router'
import './styles/index.scss'
import './styles/theme.scss'


// NProgress已移除，保持静默导航体验

const app = createApp(App)
const pinia = createPinia()

// 注册 Element Plus 图标
for (const [key, component] of Object.entries(ElementPlusIconsVue)) {
  app.component(key, component)
}

// 全局属性（Element Plus 全局配置已通过 app.use(ElementPlus, { ... }) 注入）
// 移除过时的 $ELEMENT 配置

// 全局错误处理
app.config.errorHandler = (err, vm, info) => {
  console.error('Vue Error:', err)
  console.error('Component:', vm)
  console.error('Info:', info)
}

// 全局未捕获的Promise错误处理
window.addEventListener('unhandledrejection', (event) => {
  console.error('Unhandled Promise Rejection:', event.reason)
})

// 全局错误处理
window.addEventListener('error', (event) => {
  console.error('Global Error:', event.error)
})

// 使用插件
app.use(pinia)
app.use(router)
app.use(ElementPlus, {
  locale: zhCn,
  size: 'default'
})

// 挂载应用
app.mount('#app')

// 🔍 环境变量调试信息（仅开发环境）
if (import.meta.env.VITE_ENABLE_CONSOLE_LOG === 'true' || import.meta.env.DEV) {
  console.log('='.repeat(60))
  console.log('🔍 [MAIN.JS] 环境变量调试信息:')
  console.log('  - NODE_ENV:', import.meta.env.NODE_ENV)
  console.log('  - MODE:', import.meta.env.MODE)
  console.log('  - DEV:', import.meta.env.DEV)
  console.log('  - PROD:', import.meta.env.PROD)
  console.log('  - VITE_API_BASE_URL:', import.meta.env.VITE_API_BASE_URL)
  console.log('  - VITE_API_TIMEOUT:', import.meta.env.VITE_API_TIMEOUT)
  console.log('  - VITE_APP_TITLE:', import.meta.env.VITE_APP_TITLE)
  console.log('  - VITE_APP_VERSION:', import.meta.env.VITE_APP_VERSION)
  console.log('  - VITE_DEV_PORT:', import.meta.env.VITE_DEV_PORT)
  console.log('='.repeat(60))
}

// 开发环境启动信息
if (import.meta.env.DEV) {
  console.log('🚀 MCPStore Vue Frontend Started')
  console.log('📡 API Base URL:', import.meta.env.VITE_API_BASE_URL)
  console.log('🌐 Frontend Port:', import.meta.env.VITE_DEV_PORT || '5177')
  console.log('📝 Version:', import.meta.env.VITE_APP_VERSION)
}
