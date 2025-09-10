import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '@/api'
import { useAppStore } from './app'

export const useSystemStore = defineStore('system', () => {
  const appStore = useAppStore()

  // 状态
  const services = ref([])
  const tools = ref([])
  const agents = ref([])
  const systemInfo = ref({})
  const healthStatus = ref({})
  const loading = ref(false)
  const lastUpdateTime = ref(null)

  // 统计信息
  const stats = ref({
    totalServices: 0,
    healthyServices: 0,
    unhealthyServices: 0,
    totalTools: 0,
    totalAgents: 0,
    localServices: 0,
    remoteServices: 0
  })

  // 新增状态
  const systemResources = ref({
    memory: { total: 0, used: 0, percentage: 0 },
    disk: { total: 0, used: 0, percentage: 0 },
    cpu: { usage: 0, cores: 0 },
    network: { in: 0, out: 0 }
  })

  const performanceMetrics = ref({
    apiResponseTimes: [],
    errorRates: [],
    throughput: 0,
    uptime: 0
  })

  const errors = ref([])
  const lastError = ref(null)

  // 详细加载状态
  const loadingStates = ref({
    services: false,
    tools: false,
    agents: false,
    system: false,
    health: false,
    resources: false
  })

  // 系统配置
  const systemConfig = ref({
    autoRefresh: false, // 暂时禁用自动刷新
    refreshInterval: 60000, // 增加到60秒
    healthCheckInterval: 120000, // 增加到2分钟
    maxRetries: 2 // 减少重试次数
  })
  
  // 计算属性
  const systemStatus = computed(() => ({
    isHealthy: stats.value.unhealthyServices === 0,
    healthyServices: stats.value.healthyServices,
    unhealthyServices: stats.value.unhealthyServices,
    totalServices: stats.value.totalServices,
    // 从健康状态数据中获取orchestrator状态，如果healthStatus为空则返回false
    running: healthStatus.value?.orchestrator_status === 'running'
  }))
  
  const servicesByStatus = computed(() => {
    const healthy = services.value.filter(s => s.status === 'healthy')
    const unhealthy = services.value.filter(s => s.status !== 'healthy')
    return { healthy, unhealthy }
  })
  
  const servicesByType = computed(() => {
    const local = services.value.filter(s => s.command)
    const remote = services.value.filter(s => s.url)
    return { local, remote }
  })
  
  const toolsByService = computed(() => {
    const grouped = {}
    tools.value.forEach(tool => {
      const serviceName = tool.service_name || 'unknown'
      if (!grouped[serviceName]) {
        grouped[serviceName] = []
      }
      grouped[serviceName].push(tool)
    })
    return grouped
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

  const systemHealthScore = computed(() => {
    const total = stats.value.totalServices
    const healthy = stats.value.healthyServices
    const memoryScore = 100 - systemResources.value.memory.percentage
    const diskScore = 100 - systemResources.value.disk.percentage

    if (total === 0) return 100

    const serviceScore = (healthy / total) * 100
    return Math.round((serviceScore + memoryScore + diskScore) / 3)
  })

  const resourceUsage = computed(() => {
    return {
      memory: systemResources.value.memory,
      disk: systemResources.value.disk,
      cpu: systemResources.value.cpu,
      network: systemResources.value.network
    }
  })

  const criticalServices = computed(() => {
    return services.value.filter(s => s.status === 'error' || s.status === 'unhealthy')
  })

  const availableTools = computed(() => {
    return tools.value.filter(t => t.available !== false)
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
      type: error.type || 'system-error',
      source: error.source || 'system-store'
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
  const fetchServices = async (force = false) => {
    if ((loading.value || loadingStates.value.services) && !force) return

    try {
      console.log('🔍 [STORE] 开始获取服务列表...')
      loading.value = true
      setLoadingState('services', true)
      appStore?.setLoadingState('services', true)

      const servicesArr = await api.store.listServices()
      services.value = Array.isArray(servicesArr) ? servicesArr : []

      console.log('🔍 [STORE] 解析后的服务数据:', services.value)
      console.log('🔍 [STORE] 服务数量:', services.value.length)
      updateStats()
      lastUpdateTime.value = new Date()

      console.log(`📋 Loaded ${services.value.length} services`)
      return services.value
    } catch (error) {
      console.error('❌ [STORE] 获取服务列表失败:', error)
      addError({
        message: `获取服务列表失败: ${error.message}`,
        type: 'fetch-error',
        source: 'fetchServices'
      })
      throw error
    } finally {
      loading.value = false
      setLoadingState('services', false)
      appStore?.setLoadingState('services', false)
    }
  }
  
  const fetchTools = async (force = false) => {
    if ((loading.value || loadingStates.value.tools) && !force) return

    try {
      loading.value = true
      setLoadingState('tools', true)
      appStore?.setLoadingState('tools', true)

      const toolsArr = await api.store.getTools()
      tools.value = Array.isArray(toolsArr) ? toolsArr : []
      updateStats()
      lastUpdateTime.value = new Date()

      console.log(`🛠️ Loaded ${tools.value.length} tools`)
      return tools.value
    } catch (error) {
      console.error('Failed to fetch tools:', error)
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

  const fetchAgents = async (force = false) => {
    if ((loading.value || loadingStates.value.agents) && !force) return

    try {
      loading.value = true
      setLoadingState('agents', true)
      appStore?.setLoadingState('agents', true)

      const agentsArr = await api.store.listAllAgents()
      agents.value = Array.isArray(agentsArr) ? agentsArr : []
      updateStats()
      lastUpdateTime.value = new Date()

      console.log(`🤖 Loaded ${agents.value.length} agents`)
      return agents.value
    } catch (error) {
      console.error('Failed to fetch agents:', error)
      addError({
        message: `获取代理列表失败: ${error.message}`,
        type: 'fetch-error',
        source: 'fetchAgents'
      })
      throw error
    } finally {
      loading.value = false
      setLoadingState('agents', false)
      appStore?.setLoadingState('agents', false)
    }
  }
  
  const fetchSystemStatus = async () => {
    try {
      console.log('🔍 [STORE] 开始检查服务状态...')
      loading.value = true
      const response = await api.store.checkServices()
      console.log('🔍 [STORE] 服务状态响应:', response)
      // 修复：正确提取健康状态数据
      healthStatus.value = response.data?.data || {}
      console.log('🔍 [STORE] 解析后的健康状态:', healthStatus.value)
      updateStats()
      lastUpdateTime.value = new Date()
      return healthStatus.value
    } catch (error) {
      console.error('❌ [STORE] 获取服务状态失败:', error)
      // 设置默认状态，避免无限loading
      healthStatus.value = {}
      stats.value = {
        totalServices: 0,
        healthyServices: 0,
        unhealthyServices: 0,
        totalTools: 0,
        totalAgents: 0,
        localServices: 0,
        remoteServices: 0
      }
      throw error
    } finally {
      loading.value = false
    }
  }

  // 安全的系统状态检查（静默失败）
  const safeCheckSystemStatus = async () => {
    try {
      await fetchSystemStatus()
    } catch (error) {
      // 静默失败，不抛出错误
      console.warn('System status check failed silently:', error.message)
    }
  }
  
  const addService = async (serviceConfig) => {
    try {
      loading.value = true
      const response = await api.store.addService(serviceConfig)

      // 检查添加是否成功
      if (response.data?.success) {
        // 刷新服务列表
        await fetchServices()
        await fetchTools()
        return response.data
      } else {
        // 添加失败，抛出错误
        throw new Error(response.data?.message || '服务添加失败')
      }
    } catch (error) {
      console.error('Failed to add service:', error)
      throw error
    } finally {
      loading.value = false
    }
  }
  
  const deleteService = async (serviceName) => {
    try {
      loading.value = true
      await api.store.deleteService(serviceName)
      
      // 从本地状态中移除
      services.value = services.value.filter(s => s.name !== serviceName)
      tools.value = tools.value.filter(t => t.service_name !== serviceName)
      
      updateStats()
      return true
    } catch (error) {
      console.error('Failed to delete service:', error)
      throw error
    } finally {
      loading.value = false
    }
  }
  
  const restartService = async (serviceName) => {
    try {
      loading.value = true
      await api.store.restartService(serviceName)
      
      // 刷新服务状态
      await fetchSystemStatus()
      
      return true
    } catch (error) {
      console.error('Failed to restart service:', error)
      throw error
    } finally {
      loading.value = false
    }
  }
  
  const executeToolAction = async (toolName, args) => {
    try {
      loading.value = true
      const response = await api.store.callTool(toolName, args)
      // 修复：返回正确的响应数据
      return response.data
    } catch (error) {
      console.error('Failed to execute tool:', error)
      throw error
    } finally {
      loading.value = false
    }
  }

  const getServiceInfo = async (serviceName) => {
    try {
      const response = await api.store.getServiceInfo(serviceName)
      // 修复：正确提取服务信息
      return response.data?.data
    } catch (error) {
      console.error('Failed to get service info:', error)
      throw error
    }
  }

  const updateService = async (serviceName, config) => {
    try {
      loading.value = true
      const response = await api.store.updateService(serviceName, config)

      if (response.data.success) {
        // 刷新服务列表
        await fetchServices()
        await fetchTools()
      }

      return response.data.success
    } catch (error) {
      console.error('Failed to update service:', error)
      throw error
    } finally {
      loading.value = false
    }
  }

  const patchService = async (serviceName, updates) => {
    try {
      loading.value = true
      const response = await api.store.patchService(serviceName, updates)

      if (response.data.success) {
        // 刷新服务列表
        await fetchServices()
        await fetchTools()
      }

      return response.data.success
    } catch (error) {
      console.error('Failed to patch service:', error)
      throw error
    } finally {
      loading.value = false
    }
  }

  const batchUpdateServices = async (serviceNames, updates) => {
    try {
      loading.value = true
      const response = await api.store.batchUpdateServices(serviceNames, updates)

      if (response.data.success) {
        // 刷新服务列表
        await fetchServices()
        await fetchTools()
      }

      return response.data
    } catch (error) {
      console.error('Failed to batch update services:', error)
      throw error
    } finally {
      loading.value = false
    }
  }

  const batchDeleteServices = async (serviceNames) => {
    try {
      loading.value = true
      const response = await api.store.batchDeleteServices(serviceNames)

      if (response.data.success) {
        // 从本地状态中移除
        services.value = services.value.filter(s => !serviceNames.includes(s.name))
        tools.value = tools.value.filter(t => !serviceNames.includes(t.service_name))
        updateStats()
      }

      return response.data
    } catch (error) {
      console.error('Failed to batch delete services:', error)
      throw error
    } finally {
      loading.value = false
    }
  }

  const batchRestartServices = async (serviceNames) => {
    try {
      loading.value = true
      const response = await api.store.batchRestartServices(serviceNames)

      if (response.data.success) {
        // 刷新服务状态
        await fetchServices()
        await fetchSystemStatus()
      }

      return response.data
    } catch (error) {
      console.error('Failed to batch restart services:', error)
      throw error
    } finally {
      loading.value = false
    }
  }
  
  const updateStats = () => {
    const totalServices = services.value.length
    const healthyServices = services.value.filter(s => s.status === 'healthy').length
    const unhealthyServices = totalServices - healthyServices
    const totalTools = tools.value.length
    const localServices = services.value.filter(s => s.command).length
    const remoteServices = services.value.filter(s => s.url).length

    stats.value = {
      totalServices,
      healthyServices,
      unhealthyServices,
      totalTools,
      totalAgents: agents.value.length,
      localServices,
      remoteServices
    }
  }

  const fetchToolRecords = async (limit = 50, force = false) => {
    if (loadingStates.value.resources && !force) return

    try {
      setLoadingState('resources', true)

      const response = await api.store.getToolRecords(limit)
      console.log('API响应:', response) // 调试日志

      // API返回格式: { data: { success: true, data: { executions: [...], summary: {...} }, message: "..." } }
      const apiData = response.data
      if (apiData && apiData.success && apiData.data) {
        console.log(`📊 Loaded ${apiData.data.executions.length} tool execution records`)
        return apiData.data
      } else {
        console.warn('API响应格式异常:', response)
        return { executions: [], summary: { total_executions: 0, by_tool: {}, by_service: {} } }
      }
    } catch (error) {
      console.error('获取工具执行记录失败:', error)
      addError({
        message: `获取工具执行记录失败: ${error.message}`,
        type: 'fetch-error',
        source: 'fetchToolRecords'
      })
      return { executions: [], summary: { total_executions: 0, by_tool: {}, by_service: {} } }
    } finally {
      setLoadingState('resources', false)
    }
  }

  // 获取系统资源信息
  const fetchSystemResources = async () => {
    try {
      setLoadingState('resources', true)

      const response = await api.monitoring.getSystemResources()

      if (response.success && response.data) {
        systemResources.value = {
          memory: {
            total: response.data.memory_total || 0,
            used: response.data.memory_used || 0,
            percentage: response.data.memory_percentage || 0
          },
          disk: {
            total: response.data.disk_total || 0,
            used: response.data.disk_used || 0,
            percentage: response.data.disk_usage_percentage || 0
          },
          cpu: {
            usage: response.data.cpu_usage || 0,
            cores: response.data.cpu_cores || 0
          },
          network: {
            in: response.data.network_traffic_in || 0,
            out: response.data.network_traffic_out || 0
          }
        }

        console.log('📊 System resources updated')
        return systemResources.value
      } else {
        throw new Error(response.message || 'Failed to fetch system resources')
      }

    } catch (error) {
      console.error('Failed to fetch system resources:', error)
      addError({
        message: `获取系统资源失败: ${error.message}`,
        type: 'fetch-error',
        source: 'fetchSystemResources'
      })
      return null
    } finally {
      setLoadingState('resources', false)
    }
  }
  
  const refreshAllData = async () => {
    try {
      loading.value = true
      setLoadingState('system', true)

      await Promise.all([
        fetchServices(true),
        fetchTools(true),
        fetchSystemStatus(),
        fetchSystemResources(),
        fetchToolRecords(50, true)
      ])

      lastUpdateTime.value = new Date()

      appStore?.addNotification({
        title: '数据刷新完成',
        message: '所有系统数据已更新',
        type: 'success'
      })

      console.log('🔄 All system data refreshed')
    } catch (error) {
      console.error('Failed to refresh data:', error)
      addError({
        message: `刷新系统数据失败: ${error.message}`,
        type: 'refresh-error',
        source: 'refreshAllData'
      })
      throw error
    } finally {
      loading.value = false
      setLoadingState('system', false)
    }
  }
  
  const searchServices = (query) => {
    if (!query) return services.value
    
    const lowerQuery = query.toLowerCase()
    return services.value.filter(service => 
      service.name.toLowerCase().includes(lowerQuery) ||
      (service.url && service.url.toLowerCase().includes(lowerQuery)) ||
      (service.command && service.command.toLowerCase().includes(lowerQuery))
    )
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
  
  const getServiceByName = (name) => {
    return services.value.find(service => service.name === name)
  }
  
  const getToolsByService = (serviceName) => {
    return tools.value.filter(tool => tool.service_name === serviceName)
  }
  
  const clearData = () => {
    services.value = []
    tools.value = []
    agents.value = []
    systemInfo.value = {}
    healthStatus.value = {}
    stats.value = {
      totalServices: 0,
      healthyServices: 0,
      unhealthyServices: 0,
      totalTools: 0,
      totalAgents: 0,
      localServices: 0,
      remoteServices: 0
    }
    lastUpdateTime.value = null
  }
  
  return {
    // 原有状态
    services,
    tools,
    agents,
    systemInfo,
    healthStatus,
    loading,
    lastUpdateTime,
    stats,

    // 新增状态
    systemResources,
    performanceMetrics,
    errors,
    lastError,
    loadingStates,
    systemConfig,

    // 原有计算属性
    systemStatus,
    servicesByStatus,
    servicesByType,
    toolsByService,

    // 新增计算属性
    isLoading,
    hasErrors,
    recentErrors,
    systemHealthScore,
    resourceUsage,
    criticalServices,
    availableTools,
    
    // 方法
    fetchServices,
    fetchTools,
    fetchAgents,
    fetchSystemStatus,
    safeCheckSystemStatus,
    addService,
    deleteService,
    updateService,
    patchService,
    batchUpdateServices,
    batchDeleteServices,
    batchRestartServices,
    restartService,
    executeToolAction,
    getServiceInfo,
    updateStats,
    fetchToolRecords,
    refreshAllData,
    searchServices,
    searchTools,
    getServiceByName,
    getToolsByService,
    clearData,

    // 重置Store状态
    resetStore: () => {
      services.value = []
      tools.value = []
      agents.value = []
      systemInfo.value = {}
      healthStatus.value = {}
      stats.value = {
        totalServices: 0,
        healthyServices: 0,
        unhealthyServices: 0,
        totalTools: 0,
        totalAgents: 0,
        localServices: 0,
        remoteServices: 0
      }

      // 重置新增状态
      systemResources.value = {
        memory: { total: 0, used: 0, percentage: 0 },
        disk: { total: 0, used: 0, percentage: 0 },
        cpu: { usage: 0, cores: 0 },
        network: { in: 0, out: 0 }
      }
      performanceMetrics.value = {
        apiResponseTimes: [],
        errorRates: [],
        throughput: 0,
        uptime: 0
      }
      errors.value = []
      lastError.value = null

      // 重置加载状态
      Object.keys(loadingStates.value).forEach(key => {
        loadingStates.value[key] = false
      })
      loading.value = false
      lastUpdateTime.value = null

      console.log('🔄 System store reset')
    },

    // 新增方法
    setLoadingState,
    addError,
    clearErrors,
    fetchSystemResources
  }
})
