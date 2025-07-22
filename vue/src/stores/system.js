import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { storeServiceAPI, agentServiceAPI } from '@/api/services'
import { storeMonitoringAPI } from '@/api/monitoring'

export const useSystemStore = defineStore('system', () => {
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
  
  // 计算属性
  const systemStatus = computed(() => ({
    isHealthy: stats.value.unhealthyServices === 0,
    healthyServices: stats.value.healthyServices,
    unhealthyServices: stats.value.unhealthyServices,
    totalServices: stats.value.totalServices
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
  
  // 方法
  const fetchServices = async () => {
    try {
      console.log('🔍 [STORE] 开始获取服务列表...')
      loading.value = true
      const response = await storeServiceAPI.getServices()
      console.log('🔍 [STORE] 服务列表响应:', response)
      // 修复：正确提取服务数组
      services.value = response.data?.data || []
      console.log('🔍 [STORE] 解析后的服务数据:', services.value)
      updateStats()
      lastUpdateTime.value = new Date()
      return services.value
    } catch (error) {
      console.error('❌ [STORE] 获取服务列表失败:', error)
      throw error
    } finally {
      loading.value = false
    }
  }
  
  const fetchTools = async () => {
    try {
      loading.value = true
      const response = await storeServiceAPI.getTools()
      // 修复：正确提取工具数组
      tools.value = response.data?.data || []
      updateStats()
      lastUpdateTime.value = new Date()
      return tools.value
    } catch (error) {
      console.error('Failed to fetch tools:', error)
      throw error
    } finally {
      loading.value = false
    }
  }
  
  const fetchSystemStatus = async () => {
    try {
      console.log('🔍 [STORE] 开始检查服务状态...')
      loading.value = true
      const response = await storeServiceAPI.checkServices()
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
      const response = await storeServiceAPI.addService(serviceConfig)

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
      await storeServiceAPI.deleteService(serviceName)
      
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
      await storeServiceAPI.restartService(serviceName)
      
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
      const response = await storeServiceAPI.useTool(toolName, args)
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
      const response = await storeServiceAPI.getServiceInfo(serviceName)
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
      const response = await storeServiceAPI.updateService(serviceName, config)

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
      const response = await storeServiceAPI.patchService(serviceName, updates)

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

  const batchUpdateServices = async (updates) => {
    try {
      loading.value = true
      const response = await storeServiceAPI.batchUpdateServices(updates)

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
      const response = await storeServiceAPI.batchDeleteServices(serviceNames)

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
      const response = await storeServiceAPI.batchRestartServices(serviceNames)

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

  const fetchToolRecords = async (limit = 50) => {
    try {
      const response = await storeMonitoringAPI.getToolRecords(limit)
      console.log('API响应:', response) // 调试日志

      // API返回格式: { data: { success: true, data: { executions: [...], summary: {...} }, message: "..." } }
      const apiData = response.data
      if (apiData && apiData.success && apiData.data) {
        return apiData.data
      } else {
        console.warn('API响应格式异常:', response)
        return { executions: [], summary: { total_executions: 0, by_tool: {}, by_service: {} } }
      }
    } catch (error) {
      console.error('获取工具执行记录失败:', error)
      return { executions: [], summary: { total_executions: 0, by_tool: {}, by_service: {} } }
    }
  }
  
  const refreshAllData = async () => {
    try {
      loading.value = true
      await Promise.all([
        fetchServices(),
        fetchTools(),
        fetchSystemStatus()
      ])
    } catch (error) {
      console.error('Failed to refresh data:', error)
      throw error
    } finally {
      loading.value = false
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
    // 状态
    services,
    tools,
    agents,
    systemInfo,
    healthStatus,
    loading,
    lastUpdateTime,
    stats,
    
    // 计算属性
    systemStatus,
    servicesByStatus,
    servicesByType,
    toolsByService,
    
    // 方法
    fetchServices,
    fetchTools,
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
    clearData
  }
})
