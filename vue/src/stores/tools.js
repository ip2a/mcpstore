import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '@/api'
import { useAppStore } from './app'

export const useToolsStore = defineStore('tools', () => {
  const appStore = useAppStore()

  // 状态
  const tools = ref([])
  const currentTool = ref(null)
  const executionHistory = ref([])
  const loading = ref(false)
  const executing = ref(false)
  const lastUpdateTime = ref(null)

  // 工具统计
  const stats = ref({
    total: 0,
    byService: {},
    recentExecutions: 0,
    successfulExecutions: 0,
    failedExecutions: 0
  })

  const currentExecutions = ref(new Map()) // 当前正在执行的工具
  const errors = ref([])
  const lastError = ref(null)

  // 详细加载状态
  const loadingStates = ref({
    tools: false,
    executing: false,
    details: false
  })

  // 工具配置
  const toolConfig = ref({
    autoSave: true,
    maxHistorySize: 1000,
    defaultTimeout: 30000,
    retryAttempts: 3
  })
  
  // 计算属性
  const toolsByService = computed(() => {
    return tools.value.reduce((acc, tool) => {
      const service = tool.service_name || 'unknown'
      if (!acc[service]) acc[service] = []
      acc[service].push(tool)
      return acc
    }, {})
  })
  
  const serviceNames = computed(() => {
    const names = new Set(tools.value.map(tool => tool.service_name))
    return Array.from(names).sort()
  })
  
  const recentExecutions = computed(() => {
    return executionHistory.value
      .sort((a, b) => new Date(b.timestamp) - new Date(a.timestamp))
      .slice(0, 10)
  })
  
  const popularTools = computed(() => {
    const toolCounts = {}
    executionHistory.value.forEach(execution => {
      toolCounts[execution.toolName] = (toolCounts[execution.toolName] || 0) + 1
    })
    
    return Object.entries(toolCounts)
      .sort(([,a], [,b]) => b - a)
      .slice(0, 10)
      .map(([toolName, count]) => ({
        name: toolName,
        count,
        tool: tools.value.find(t => t.name === toolName)
      }))
  })

  // 新增计算属性
  const isLoading = computed(() => {
    return Object.values(loadingStates.value).some(Boolean) || loading.value
  })

  const hasErrors = computed(() => {
    return errors.value.length > 0
  })

  const recentErrors = computed(() => {
    return errors.value.slice(-5).reverse()
  })

  const isExecuting = computed(() => {
    return currentExecutions.value.size > 0 || executing.value
  })

  const toolsByCategory = computed(() => {
    const categories = {}
    tools.value.forEach(tool => {
      const category = tool.category || 'uncategorized'
      if (!categories[category]) categories[category] = []
      categories[category].push(tool)
    })
    return categories
  })

  const availableTools = computed(() => {
    return tools.value.filter(tool => tool.available !== false)
  })

  const favoriteTools = computed(() => {
    return tools.value.filter(tool => tool.favorite === true)
  })
  
  // 新增方法
  const setLoadingState = (type, status) => {
    if (type in loadingStates.value) {
      loadingStates.value[type] = status
    }
  }

  const addError = (error) => {
    const errorObj = {
      id: Date.now(),
      message: error.message || error,
      timestamp: new Date().toISOString(),
      type: error.type || 'tool-error',
      source: error.source || 'tools-store'
    }

    errors.value.push(errorObj)
    lastError.value = errorObj

    // 限制错误数量
    if (errors.value.length > 50) {
      errors.value = errors.value.slice(-50)
    }

    // 同时添加到应用级错误
    if (appStore) {
      appStore.addError(errorObj)
    }
  }

  const clearErrors = () => {
    errors.value = []
    lastError.value = null
  }

  // 方法
  const fetchTools = async (force = false) => {
    if ((loading.value || loadingStates.value.tools) && !force) return

    loading.value = true
    setLoadingState('tools', true)

    try {
      appStore?.setLoadingState('tools', true)

      const response = await api.store.listTools()

      // 🔍 调试：检查API返回的数据格式
      console.log('🔍 [DEBUG] Tools API返回的原始数据:', response)
      console.log('🔍 [DEBUG] response.data类型:', typeof response.data)
      console.log('🔍 [DEBUG] response.data是否为数组:', Array.isArray(response.data))

      // 🔧 修复：正确处理API响应格式
      let toolsData = []

      if (response.data && response.data.success && Array.isArray(response.data.data)) {
        // 新格式：{ success: true, data: [...] }
        toolsData = response.data.data
        console.log('✅ [DEBUG] 使用新格式 response.data.data')
      } else if (Array.isArray(response.data)) {
        // 直接数组格式
        toolsData = response.data
        console.log('✅ [DEBUG] 使用 response.data (直接数组)')
      } else if (Array.isArray(response)) {
        // 响应本身是数组
        toolsData = response
        console.log('✅ [DEBUG] 使用 response (直接数组)')
      } else if (response.data && Array.isArray(response.data.tools)) {
        // 嵌套格式：{ data: { tools: [...] } }
        toolsData = response.data.tools
        console.log('✅ [DEBUG] 使用 response.data.tools')
      } else {
        console.warn('⚠️ [DEBUG] 无法识别的Tools API响应格式')
        console.warn('响应结构:', {
          hasData: !!response.data,
          hasSuccess: !!(response.data && response.data.success),
          hasDataData: !!(response.data && response.data.data),
          dataType: typeof response.data,
          dataDataType: response.data && typeof response.data.data
        })
        toolsData = []
      }

      // 确保每个工具都有必要的字段
      tools.value = toolsData.map(tool => ({
        ...tool,
        available: tool.available !== false, // 默认为可用
        favorite: tool.favorite || false,
        category: tool.category || 'default'
      }))

      console.log('🔍 [DEBUG] 提取的工具数据:', toolsData)
      console.log('🔍 [DEBUG] 处理后的tools.value:', tools.value)
      console.log('🔍 [DEBUG] 工具数量:', tools.value.length)
      console.log('🔍 [DEBUG] 可用工具数量:', tools.value.filter(t => t.available !== false).length)

      updateStats()
      lastUpdateTime.value = new Date()

      console.log(`🛠️ Loaded ${tools.value.length} tools`)
      return tools.value
    } catch (error) {
      console.error('获取工具列表失败:', error)
      addError({
        message: `获取工具列表失败: ${error.message}`,
        type: 'fetch-error',
        source: 'fetchTools'
      })
      throw error
    } finally {
      loading.value = false
      setLoadingState('tools', false)
      appStore?.setLoadingState('tools', false)
    }
  }
  
  const executeTool = async (toolName, params) => {
    const executionId = `${toolName}_${Date.now()}`

    try {
      executing.value = true
      setLoadingState('executing', true)

      // 记录开始执行
      currentExecutions.value.set(executionId, {
        toolName,
        params,
        startTime: Date.now(),
        status: 'running'
      })

      const startTime = Date.now()
      const response = await api.store.callTool(toolName, params)
      const endTime = Date.now()
      const duration = endTime - startTime

      // 添加到执行历史
      const execution = {
        id: Date.now(),
        toolName,
        params,
        result: response.data,
        success: response.data.success !== false,
        timestamp: new Date().toISOString(),
        duration,
        message: response.data.message || ''
      }

      executionHistory.value.unshift(execution)

      // 限制历史记录数量
      if (executionHistory.value.length > toolConfig.value.maxHistorySize) {
        executionHistory.value = executionHistory.value.slice(0, toolConfig.value.maxHistorySize)
      }

      updateStats()

      // 添加成功通知
      if (execution.success) {
        appStore?.addNotification({
          title: '工具执行成功',
          message: `工具 "${toolName}" 执行完成`,
          type: 'success'
        })
      }

      return response
    } catch (error) {
      const endTime = Date.now()
      const duration = endTime - startTime

      // 添加失败的执行记录
      const execution = {
        id: Date.now(),
        toolName,
        params,
        result: null,
        success: false,
        timestamp: new Date().toISOString(),
        duration,
        message: error.message || '执行失败'
      }

      executionHistory.value.unshift(execution)
      updateStats()

      // 添加错误
      addError({
        message: `工具执行失败: ${error.message}`,
        type: 'execution-error',
        source: 'executeTool',
        toolName
      })

      throw error
    } finally {
      executing.value = false
      setLoadingState('executing', false)
      currentExecutions.value.delete(executionId)
    }
  }
  
  const getToolDetails = async (toolName) => {
    try {
      const response = await api.store.getToolInfo(toolName)
      return response.data
    } catch (error) {
      console.error('获取工具详情失败:', error)
      throw error
    }
  }
  

  // 标记工具为收藏
  const toggleToolFavorite = (toolName) => {
    const tool = tools.value.find(t => t.name === toolName)
    if (tool) {
      tool.favorite = !tool.favorite

      // 保存到localStorage
      const favorites = JSON.parse(localStorage.getItem('mcpstore-favorite-tools') || '[]')
      if (tool.favorite) {
        if (!favorites.includes(toolName)) {
          favorites.push(toolName)
        }
      } else {
        const index = favorites.indexOf(toolName)
        if (index > -1) {
          favorites.splice(index, 1)
        }
      }
      localStorage.setItem('mcpstore-favorite-tools', JSON.stringify(favorites))
    }
  }

  // 加载收藏工具
  const loadFavoriteTools = () => {
    try {
      const favorites = JSON.parse(localStorage.getItem('mcpstore-favorite-tools') || '[]')
      tools.value.forEach(tool => {
        tool.favorite = favorites.includes(tool.name)
      })
    } catch (error) {
      console.warn('Failed to load favorite tools:', error)
    }
  }
  
  const updateStats = () => {
    // 安全检查：确保tools.value是数组
    if (!Array.isArray(tools.value)) {
      console.warn('⚠️ updateStats: tools.value不是数组，跳过统计更新')
      return
    }

    stats.value.total = tools.value.length

    // 按服务统计
    stats.value.byService = {}
    tools.value.forEach(tool => {
      const service = tool.service_name || 'unknown'
      stats.value.byService[service] = (stats.value.byService[service] || 0) + 1
    })

    // 执行统计
    stats.value.recentExecutions = executionHistory.value.length
    stats.value.successfulExecutions = executionHistory.value.filter(e => e.success).length
    stats.value.failedExecutions = executionHistory.value.filter(e => !e.success).length
  }
  
  const setCurrentTool = (tool) => {
    currentTool.value = tool
  }
  
  const getToolByName = (name) => {
    return tools.value.find(t => t.name === name)
  }
  
  const getToolsByService = (serviceName) => {
    return tools.value.filter(t => t.service_name === serviceName)
  }
  
  const searchTools = (query) => {
    if (!query) return tools.value
    
    const lowerQuery = query.toLowerCase()
    return tools.value.filter(tool => 
      tool.name.toLowerCase().includes(lowerQuery) ||
      (tool.description && tool.description.toLowerCase().includes(lowerQuery)) ||
      (tool.service_name && tool.service_name.toLowerCase().includes(lowerQuery))
    )
  }
  
  const clearExecutionHistory = () => {
    executionHistory.value = []
    updateStats()
  }
  
  const removeExecutionFromHistory = (executionId) => {
    const index = executionHistory.value.findIndex(e => e.id === executionId)
    if (index > -1) {
      executionHistory.value.splice(index, 1)
      updateStats()
    }
  }
  
  const resetStore = () => {
    tools.value = []
    currentTool.value = null
    executionHistory.value = []
    stats.value = {
      total: 0,
      byService: {},
      recentExecutions: 0,
      successfulExecutions: 0,
      failedExecutions: 0
    }
    lastUpdateTime.value = null

    // 重置新增状态
    currentExecutions.value.clear()
    errors.value = []
    lastError.value = null

    // 重置加载状态
    Object.keys(loadingStates.value).forEach(key => {
      loadingStates.value[key] = false
    })
    loading.value = false
    executing.value = false

    console.log('🔄 Tools store reset')
  }
  
  return {
    // 原有状态
    tools,
    currentTool,
    executionHistory,
    loading,
    executing,
    lastUpdateTime,
    stats,

    currentExecutions,
    errors,
    lastError,
    loadingStates,
    toolConfig,

    // 原有计算属性
    toolsByService,
    serviceNames,
    recentExecutions,
    popularTools,

    // 新增计算属性
    isLoading,
    hasErrors,
    recentErrors,
    isExecuting,
    toolsByCategory,
    availableTools,
    favoriteTools,

    // 原有方法
    fetchTools,
    executeTool,
    getToolDetails,
    updateStats,
    setCurrentTool,
    getToolByName,
    getToolsByService,
    searchTools,
    clearExecutionHistory,
    removeExecutionFromHistory,
    resetStore,

    // 新增方法
    setLoadingState,
    addError,
    clearErrors,
    toggleToolFavorite,
    loadFavoriteTools
  }
})
