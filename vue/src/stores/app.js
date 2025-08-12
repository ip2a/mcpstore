import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

export const useAppStore = defineStore('app', () => {
  // 状态
  const isCollapse = ref(false)
  const theme = ref('light')
  const language = ref('zh-CN')
  const loading = ref(false)
  
  // 设备信息
  const device = ref('desktop')
  const isMobile = computed(() => device.value === 'mobile')
  const isTablet = computed(() => device.value === 'tablet')
  const isDesktop = computed(() => device.value === 'desktop')
  
  // 布局配置
  const layoutConfig = ref({
    sidebarWidth: 250,
    sidebarCollapsedWidth: 64,
    headerHeight: 60,
    footerHeight: 50,
    showFooter: false,
    showBreadcrumb: true,
    showTabs: false
  })
  
  // 主题配置
  const themeConfig = ref({
    primaryColor: '#409EFF',
    successColor: '#67C23A',
    warningColor: '#E6A23C',
    dangerColor: '#F56C6C',
    infoColor: '#909399'
  })
  
  // 用户偏好设置
  const userPreferences = ref({
    autoRefresh: false, // 暂时禁用自动刷新
    refreshInterval: 60000, // 增加到60秒
    showNotifications: true,
    soundEnabled: false,
    animationEnabled: true,
    dashboardLayout: 'default', // 'default' | 'compact' | 'detailed'
    toolDisplayMode: 'grid', // 'grid' | 'list'
    pageSize: 20
  })

  // 应用配置
  const config = ref({
    apiBaseUrl: import.meta.env.VITE_API_BASE_URL || 'http://localhost:18200',
    apiTimeout: parseInt(import.meta.env.VITE_API_TIMEOUT) || 30000,
    appTitle: import.meta.env.VITE_APP_TITLE || 'MCPStore',
    version: '1.0.0',
    environment: import.meta.env.MODE || 'development'
  })

  // 全局加载状态
  const loadingStates = ref({
    global: false,
    api: false,
    tools: false,
    services: false,
    dashboard: false
  })

  // 错误状态管理
  const errors = ref([])
  const lastError = ref(null)

  // 通知状态
  const notifications = ref([])
  const unreadCount = ref(0)

  // 应用状态
  const appState = ref({
    initialized: false,
    connected: true,
    lastActivity: Date.now(),
    sessionId: null,
    uptime: 0
  })

  // 性能监控
  const performance = ref({
    apiResponseTimes: [],
    memoryUsage: 0,
    renderTime: 0,
    errorCount: 0
  })
  
  // 计算属性
  const isDark = computed(() => theme.value === 'dark')
  const sidebarWidth = computed(() =>
    isCollapse.value ? layoutConfig.value.sidebarCollapsedWidth : layoutConfig.value.sidebarWidth
  )

  // 是否有任何加载状态
  const isLoading = computed(() => {
    return Object.values(loadingStates.value).some(Boolean)
  })

  // 是否有错误
  const hasErrors = computed(() => {
    return errors.value.length > 0
  })

  // 是否为开发环境
  const isDevelopment = computed(() => {
    return config.value.environment === 'development'
  })

  // 应用是否就绪
  const isReady = computed(() => {
    return appState.value.initialized && appState.value.connected && !isLoading.value
  })

  // 最近的错误
  const recentErrors = computed(() => {
    return errors.value.slice(-5).reverse()
  })

  // 未读通知数量
  const hasUnreadNotifications = computed(() => {
    return unreadCount.value > 0
  })
  
  // 方法
  const setCollapse = (value) => {
    isCollapse.value = value
    localStorage.setItem('mcpstore-collapse', value.toString())
  }
  
  const setTheme = (value) => {
    theme.value = value
    localStorage.setItem('mcpstore-theme', value)
    
    // 更新CSS变量
    const root = document.documentElement
    if (value === 'dark') {
      root.classList.add('dark')
    } else {
      root.classList.remove('dark')
    }
  }
  
  const setLanguage = (value) => {
    language.value = value
    localStorage.setItem('mcpstore-language', value)
  }
  
  const setDevice = (value) => {
    device.value = value
    
    // 移动端自动收起侧边栏
    if (value === 'mobile') {
      setCollapse(true)
    }
  }
  
  const setLoading = (value) => {
    loading.value = value
  }

  // 设置特定类型的加载状态
  const setLoadingState = (type, status) => {
    if (type in loadingStates.value) {
      loadingStates.value[type] = status
    }
  }

  // 设置全局加载状态
  const setGlobalLoading = (status) => {
    loadingStates.value.global = status
  }

  // 添加错误
  const addError = (error) => {
    const errorObj = {
      id: Date.now(),
      message: error.message || error,
      stack: error.stack,
      timestamp: new Date().toISOString(),
      type: error.type || 'error',
      source: error.source || 'unknown'
    }

    errors.value.push(errorObj)
    lastError.value = errorObj
    performance.value.errorCount++

    // 限制错误数量，只保留最近100个
    if (errors.value.length > 100) {
      errors.value = errors.value.slice(-100)
    }
  }

  // 清除错误
  const clearErrors = () => {
    errors.value = []
    lastError.value = null
  }

  // 移除特定错误
  const removeError = (errorId) => {
    const index = errors.value.findIndex(error => error.id === errorId)
    if (index > -1) {
      errors.value.splice(index, 1)
    }
  }
  
  // setPageLoading已移除，不再需要全局页面loading
  
  const updateLayoutConfig = (config) => {
    layoutConfig.value = { ...layoutConfig.value, ...config }
    localStorage.setItem('mcpstore-layout', JSON.stringify(layoutConfig.value))
  }
  
  const updateThemeConfig = (config) => {
    themeConfig.value = { ...themeConfig.value, ...config }
    localStorage.setItem('mcpstore-theme-config', JSON.stringify(themeConfig.value))
    
    // 更新CSS变量
    const root = document.documentElement
    Object.entries(config).forEach(([key, value]) => {
      const cssVar = `--el-color-${key.replace('Color', '')}`
      root.style.setProperty(cssVar, value)
    })
  }
  
  const updateUserPreferences = (preferences) => {
    userPreferences.value = { ...userPreferences.value, ...preferences }
    localStorage.setItem('mcpstore-preferences', JSON.stringify(userPreferences.value))
  }

  // 添加通知
  const addNotification = (notification) => {
    const notificationObj = {
      id: Date.now(),
      title: notification.title,
      message: notification.message,
      type: notification.type || 'info', // 'success' | 'warning' | 'error' | 'info'
      timestamp: new Date().toISOString(),
      read: false,
      persistent: notification.persistent || false
    }

    notifications.value.unshift(notificationObj)
    unreadCount.value++

    // 限制通知数量
    if (notifications.value.length > 50) {
      notifications.value = notifications.value.slice(0, 50)
    }
  }

  // 标记通知为已读
  const markNotificationRead = (notificationId) => {
    const notification = notifications.value.find(n => n.id === notificationId)
    if (notification && !notification.read) {
      notification.read = true
      unreadCount.value = Math.max(0, unreadCount.value - 1)
    }
  }

  // 清除所有通知
  const clearNotifications = () => {
    notifications.value = []
    unreadCount.value = 0
  }

  // 更新连接状态
  const setConnectionStatus = (connected) => {
    appState.value.connected = connected
    if (!connected) {
      addNotification({
        title: '连接断开',
        message: '与服务器的连接已断开，正在尝试重连...',
        type: 'warning',
        persistent: true
      })
    }
  }

  // 记录API响应时间
  const recordApiResponseTime = (time) => {
    performance.value.apiResponseTimes.push({
      time,
      timestamp: Date.now()
    })

    // 只保留最近100次记录
    if (performance.value.apiResponseTimes.length > 100) {
      performance.value.apiResponseTimes = performance.value.apiResponseTimes.slice(-100)
    }
  }

  // 更新活动时间
  const updateActivity = () => {
    appState.value.lastActivity = Date.now()
  }
  
  const initializeApp = async () => {
    try {
      setGlobalLoading(true)

      // 从localStorage恢复状态
      const savedCollapse = localStorage.getItem('mcpstore-collapse')
      if (savedCollapse !== null) {
        isCollapse.value = savedCollapse === 'true'
      }

      const savedTheme = localStorage.getItem('mcpstore-theme')
      if (savedTheme) {
        setTheme(savedTheme)
      }

      const savedLanguage = localStorage.getItem('mcpstore-language')
      if (savedLanguage) {
        language.value = savedLanguage
      }

      const savedLayout = localStorage.getItem('mcpstore-layout')
      if (savedLayout) {
        try {
          layoutConfig.value = { ...layoutConfig.value, ...JSON.parse(savedLayout) }
        } catch (e) {
          console.warn('Failed to parse saved layout config:', e)
        }
      }

      const savedThemeConfig = localStorage.getItem('mcpstore-theme-config')
      if (savedThemeConfig) {
        try {
          updateThemeConfig(JSON.parse(savedThemeConfig))
        } catch (e) {
          console.warn('Failed to parse saved theme config:', e)
        }
      }

      const savedPreferences = localStorage.getItem('mcpstore-preferences')
      if (savedPreferences) {
        try {
          userPreferences.value = { ...userPreferences.value, ...JSON.parse(savedPreferences) }
        } catch (e) {
          console.warn('Failed to parse saved preferences:', e)
        }
      }

      // 生成会话ID
      appState.value.sessionId = `session_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`

      // 检测设备类型
      detectDevice()

      // 监听窗口大小变化
      window.addEventListener('resize', detectDevice)

      // 标记为已初始化
      appState.value.initialized = true
      appState.value.lastActivity = Date.now()

      console.log('🚀 App initialized successfully')

    } catch (error) {
      addError({
        message: 'Failed to initialize app',
        source: 'app-store',
        type: 'initialization',
        ...error
      })
    } finally {
      setGlobalLoading(false)
    }
  }
  
  const detectDevice = () => {
    const width = window.innerWidth
    if (width < 768) {
      setDevice('mobile')
    } else if (width < 1024) {
      setDevice('tablet')
    } else {
      setDevice('desktop')
    }
  }
  
  const saveSettings = () => {
    // 保存所有设置到localStorage
    localStorage.setItem('mcpstore-collapse', isCollapse.value.toString())
    localStorage.setItem('mcpstore-theme', theme.value)
    localStorage.setItem('mcpstore-language', language.value)
    localStorage.setItem('mcpstore-layout', JSON.stringify(layoutConfig.value))
    localStorage.setItem('mcpstore-theme-config', JSON.stringify(themeConfig.value))
    localStorage.setItem('mcpstore-preferences', JSON.stringify(userPreferences.value))
    console.log('✅ Settings saved to localStorage')
  }

  const resetSettings = () => {
    // 重置为默认值
    isCollapse.value = false
    theme.value = 'light'
    language.value = 'zh-CN'
    layoutConfig.value = {
      sidebarWidth: 250,
      sidebarCollapsedWidth: 64,
      headerHeight: 60,
      footerHeight: 50,
      showFooter: false,
      showBreadcrumb: true,
      showTabs: false
    }
    themeConfig.value = {
      primaryColor: '#409EFF',
      successColor: '#67C23A',
      warningColor: '#E6A23C',
      dangerColor: '#F56C6C',
      infoColor: '#909399'
    }
    userPreferences.value = {
      autoRefresh: false, // 默认禁用自动刷新
      refreshInterval: 60000, // 60秒
      showNotifications: true,
      soundEnabled: false,
      animationEnabled: true,
      dashboardLayout: 'default',
      toolDisplayMode: 'grid',
      pageSize: 20
    }

    // 清除所有状态
    errors.value = []
    notifications.value = []
    unreadCount.value = 0
    lastError.value = null

    // 重置加载状态
    Object.keys(loadingStates.value).forEach(key => {
      loadingStates.value[key] = false
    })

    // 重置性能数据
    performance.value = {
      apiResponseTimes: [],
      memoryUsage: 0,
      renderTime: 0,
      errorCount: 0
    }

    // 清除localStorage
    localStorage.removeItem('mcpstore-collapse')
    localStorage.removeItem('mcpstore-theme')
    localStorage.removeItem('mcpstore-language')
    localStorage.removeItem('mcpstore-layout')
    localStorage.removeItem('mcpstore-theme-config')
    localStorage.removeItem('mcpstore-preferences')

    // 重新应用设置
    setTheme('light')

    console.log('🔄 App settings reset')
  }
  
  return {
    // 原有状态
    isCollapse,
    theme,
    language,
    loading,
    device,
    layoutConfig,
    themeConfig,
    userPreferences,

    // 新增状态
    config,
    loadingStates,
    errors,
    lastError,
    notifications,
    unreadCount,
    appState,
    performance,

    // 原有计算属性
    isDark,
    isMobile,
    isTablet,
    isDesktop,
    sidebarWidth,

    // 新增计算属性
    isLoading,
    hasErrors,
    isDevelopment,
    isReady,
    recentErrors,
    hasUnreadNotifications,

    // 原有方法
    setCollapse,
    setTheme,
    setLanguage,
    setDevice,
    setLoading,
    updateLayoutConfig,
    updateThemeConfig,
    updateUserPreferences,
    initializeApp,
    detectDevice,
    saveSettings,
    resetSettings,

    // 新增方法
    setLoadingState,
    setGlobalLoading,
    addError,
    clearErrors,
    removeError,
    addNotification,
    markNotificationRead,
    clearNotifications,
    setConnectionStatus,
    recordApiResponseTime,
    updateActivity
  }
})
