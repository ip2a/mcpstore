import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { storeServiceAPI, agentServiceAPI } from '@/api/services'

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
      loading.value = true
      const response = await storeServiceAPI.getServices()
      services.value = response.data || []
      updateStats()
      lastUpdateTime.value = new Date()
      return services.value
    } catch (error) {
      console.error('Failed to fetch services:', error)
      throw error
    } finally {
      loading.value = false
    }
  }
  
  const fetchTools = async () => {
    try {
      loading.value = true
      const response = await storeServiceAPI.getTools()
      tools.value = response.data || []
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
      loading.value = true
      const response = await storeServiceAPI.checkServices()
      healthStatus.value = response.data || {}
      updateStats()
      lastUpdateTime.value = new Date()
      return healthStatus.value
    } catch (error) {
      console.error('Failed to fetch system status:', error)
      throw error
    } finally {
      loading.value = false
    }
  }
  
  const addService = async (serviceConfig) => {
    try {
      loading.value = true
      const response = await storeServiceAPI.addService(serviceConfig)
      
      // 刷新服务列表
      await fetchServices()
      await fetchTools()
      
      return response
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
      return response
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
      return response.data
    } catch (error) {
      console.error('Failed to get service info:', error)
      throw error
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
    addService,
    deleteService,
    restartService,
    executeToolAction,
    getServiceInfo,
    updateStats,
    refreshAllData,
    searchServices,
    searchTools,
    getServiceByName,
    getToolsByService,
    clearData
  }
})
