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

// NProgress已移除，保持静默导航体验

const app = createApp(App)
const pinia = createPinia()

// 注册 Element Plus 图标
for (const [key, component] of Object.entries(ElementPlusIconsVue)) {
  app.component(key, component)
}

// 全局属性
app.config.globalProperties.$ELEMENT = {
  size: 'default',
  zIndex: 3000
}

// 全局错误处理
app.config.errorHandler = (err, vm, info) => {
  console.error('Vue Error:', err)
  console.error('Component:', vm)
  console.error('Info:', info)
}

// 全局未捕获的Promise错误处理
window.addEventListener('unhandledrejection', (event) => {
  console.error('Unhandled Promise Rejection:', event.reason)
  // 防止默认的控制台错误输出
  event.preventDefault()
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

// 🔍 环境变量调试信息（总是显示）
console.log('='.repeat(60))
console.log('🔍 [MAIN.JS] 环境变量调试信息:')
console.log('  - NODE_ENV:', import.meta.env.NODE_ENV)
console.log('  - MODE:', import.meta.env.MODE)
console.log('  - DEV:', import.meta.env.DEV)
console.log('  - PROD:', import.meta.env.PROD)
console.log('  - VITE_API_BASE_URL:', import.meta.env.VITE_API_BASE_URL)
console.log('  - VITE_API_TIMEOUT:', import.meta.env.VITE_API_TIMEOUT)
console.log('  - VITE_APP_TITLE:', import.meta.env.VITE_APP_TITLE)
console.log('  - 完整环境变量对象:', import.meta.env)
console.log('='.repeat(60))

// 开发环境下的调试信息
if (import.meta.env.DEV) {
  console.log('🚀 MCPStore Vue Frontend Started')
  console.log('📡 API Base URL:', import.meta.env.VITE_API_BASE_URL || 'http://localhost:18200')
  console.log('🌐 Frontend Port:', 5177)
}
