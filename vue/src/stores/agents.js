import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '@/api'

export const useAgentsStore = defineStore('agents', () => {
  // 状态
  const agents = ref([])
  const currentAgent = ref(null)
  const loading = ref(false)
  const lastUpdateTime = ref(null)
  
  // Agent统计
  const stats = ref({
    total: 0,
    active: 0,
    inactive: 0,
    partial: 0,
    error: 0,
    totalServices: 0,
    totalTools: 0
  })
  
  // 计算属性
  const agentsByStatus = computed(() => {
    return agents.value.reduce((acc, agent) => {
      const status = agent.status || 'inactive'
      if (!acc[status]) acc[status] = []
      acc[status].push(agent)
      return acc
    }, {})
  })
  
  const activeAgents = computed(() => {
    return agents.value.filter(a => a.status === 'active')
  })
  
  const inactiveAgents = computed(() => {
    return agents.value.filter(a => a.status === 'inactive')
  })
  
  const partialAgents = computed(() => {
    return agents.value.filter(a => a.status === 'partial')
  })
  
  const errorAgents = computed(() => {
    return agents.value.filter(a => a.status === 'error')
  })
  
  // === 核心数据获取 ===
  
  const fetchAgents = async () => {
    loading.value = true
    try {
      console.log('Fetching agents from API...')
      const response = await api.agent.getAgentsList()
      console.log('API response:', response)
      console.log('Response structure:', Object.keys(response))

      // 修复：正确访问嵌套的data结构
      const responseData = response.data || {}
      console.log('Response data:', responseData)
      const agentsSummary = responseData.data || responseData || {}
      console.log('Agents summary:', agentsSummary)
      console.log('Raw agents array:', agentsSummary.agents)
      console.log('Agents array type:', typeof agentsSummary.agents)
      console.log('Agents array length:', agentsSummary.agents?.length)

      // 转换后端数据格式为前端需要的格式
      const rawAgents = agentsSummary.agents || []
      console.log('Raw agents before map:', rawAgents)

      if (!Array.isArray(rawAgents)) {
        console.error('Agents is not an array:', rawAgents)
        agents.value = []
      } else {
        agents.value = rawAgents.map((agent, index) => {
          console.log(`Processing agent ${index}:`, agent)
          try {
            const status = getAgentStatus(agent)
            console.log('Agent status:', status)
            const processedAgent = {
              id: agent.agent_id,
              name: agent.agent_id, // Agent ID就是名称
              description: `Agent with ${agent.service_count} services`,
              status: status,
              services: agent.service_count,
              tools: agent.tool_count,
              healthy_services: agent.healthy_services,
              unhealthy_services: agent.unhealthy_services,
              total_tool_executions: agent.total_tool_executions,
              last_activity: agent.last_activity,
              created_at: agent.last_activity || new Date().toISOString()
            }
            console.log('Processed agent:', processedAgent)
            return processedAgent
          } catch (error) {
            console.error('Error processing agent:', error)
            return null
          }
        }).filter(agent => agent !== null)
      }

      console.log('Processed agents:', agents.value)
      updateStats()
      lastUpdateTime.value = new Date()
      return agents.value
    } catch (error) {
      console.error('获取Agent列表失败:', error)
      // 如果API不可用，使用空数据
      agents.value = []
      updateStats()
      throw error
    } finally {
      loading.value = false
    }
  }
  
  // 根据服务健康状态确定Agent状态
  const getAgentStatus = (agent) => {
    if (agent.service_count === 0) return 'inactive'
    if (agent.healthy_services === agent.service_count && agent.healthy_services > 0) return 'active'
    if (agent.healthy_services > 0) return 'partial'
    // 如果有服务但健康状态未知，显示为部分可用而不是非活跃
    if (agent.service_count > 0 && agent.healthy_services === 0 && agent.unhealthy_services === 0) {
      return 'partial' // 状态未知，但有服务
    }
    return 'inactive'
  }
  
  // === Agent服务管理 ===
  
  const getAgentServices = async (agentId) => {
    try {
      console.log('🔍 [DEBUG] 获取Agent服务列表:', agentId)
      const response = await api.agent.getAgentServices(agentId)
      console.log('🔍 [DEBUG] Agent服务API响应:', response)

      // 🔧 修复：正确处理API响应格式
      let services = []
      if (response.data && response.data.success && Array.isArray(response.data.data)) {
        services = response.data.data
        console.log('✅ [DEBUG] 使用 response.data.data (数组)')
      } else if (Array.isArray(response.data)) {
        services = response.data
        console.log('✅ [DEBUG] 使用 response.data (直接数组)')
      } else {
        console.warn('⚠️ [DEBUG] 无法识别的服务API响应格式')
        services = []
      }

      console.log('🔍 [DEBUG] 提取的服务数据:', services)
      return services
    } catch (error) {
      console.error('获取Agent服务列表失败:', error)
      throw error
    }
  }

  const getAgentTools = async (agentId) => {
    try {
      console.log('🔍 [DEBUG] 获取Agent工具列表:', agentId)
      const response = await api.agent.getAgentTools(agentId)
      console.log('🔍 [DEBUG] Agent工具API响应:', response)

      // 🔧 修复：正确处理API响应格式
      let tools = []
      if (response.data && response.data.success && Array.isArray(response.data.data)) {
        tools = response.data.data
        console.log('✅ [DEBUG] 使用 response.data.data (数组)')
      } else if (Array.isArray(response.data)) {
        tools = response.data
        console.log('✅ [DEBUG] 使用 response.data (直接数组)')
      } else {
        console.warn('⚠️ [DEBUG] 无法识别的工具API响应格式')
        tools = []
      }

      console.log('🔍 [DEBUG] 提取的工具数据:', tools)
      return tools
    } catch (error) {
      console.error('获取Agent工具列表失败:', error)
      throw error
    }
  }

  const getAgentStats = async (agentId) => {
    try {
      console.log('🔍 [DEBUG] 获取Agent统计信息:', agentId)
      const response = await api.agent.getAgentStats(agentId)
      console.log('🔍 [DEBUG] Agent统计API响应:', response)

      // 🔧 修复：正确处理API响应格式并映射字段
      let stats = {}
      if (response.data && response.data.success && response.data.data) {
        const data = response.data.data

        // 映射API响应字段到组件期望的字段
        stats = {
          services: data.services?.total || 0,
          tools: data.tools?.total || 0,
          healthy_services: data.services?.healthy || 0,
          unhealthy_services: data.services?.unhealthy || 0,
          total_tool_executions: data.tools?.total_executions || 0,
          orchestrator_status: data.system?.orchestrator_status || 'unknown',
          by_transport: data.services?.by_transport || {}
        }
        console.log('✅ [DEBUG] 映射后的统计数据:', stats)
      } else if (response.data && typeof response.data === 'object') {
        stats = response.data
        console.log('✅ [DEBUG] 使用原始统计数据:', stats)
      } else {
        console.warn('⚠️ [DEBUG] 无法识别的统计API响应格式')
        stats = {
          services: 0,
          tools: 0,
          healthy_services: 0,
          unhealthy_services: 0,
          total_tool_executions: 0,
          orchestrator_status: 'unknown'
        }
      }

      return stats
    } catch (error) {
      console.error('获取Agent统计信息失败:', error)
      throw error
    }
  }
  
  const addService = async (agentId, serviceConfig) => {
    try {
      const response = await api.agent.addService(agentId, serviceConfig)
      if (response.data.success) {
        await fetchAgents() // 重新获取列表以更新统计
        return { success: true, data: response.data }
      } else {
        return { success: false, error: response.data.message }
      }
    } catch (error) {
      return { success: false, error: error.message }
    }
  }
  
  const deleteService = async (agentId, serviceName) => {
    try {
      const response = await api.agent.deleteService(agentId, serviceName)
      if (response.data.success) {
        await fetchAgents() // 重新获取列表以更新统计
        return { success: true }
      } else {
        return { success: false, error: response.data.message }
      }
    } catch (error) {
      return { success: false, error: error.message }
    }
  }
  
  const updateService = async (agentId, serviceName, config) => {
    try {
      const response = await api.agent.updateService(agentId, serviceName, config)
      if (response.data.success) {
        await fetchAgents() // 重新获取列表以更新统计
        return { success: true, data: response.data }
      } else {
        return { success: false, error: response.data.message }
      }
    } catch (error) {
      return { success: false, error: error.message }
    }
  }
  
  const restartService = async (agentId, serviceName) => {
    try {
      const response = await api.agent.restartService(agentId, serviceName)
      return response.data
    } catch (error) {
      console.error('重启服务失败:', error)
      throw error
    }
  }
  
  const useTool = async (agentId, toolName, args) => {
    try {
      const response = await api.agent.callTool(agentId, toolName, args)
      return response.data
    } catch (error) {
      console.error('使用工具失败:', error)
      throw error
    }
  }
  
  const checkServices = async (agentId) => {
    try {
      const response = await api.agent.checkServices(agentId)
      return response.data
    } catch (error) {
      console.error('检查服务健康状态失败:', error)
      throw error
    }
  }
  
  const resetAgentConfig = async (agentId) => {
    try {
      const response = await api.agent.resetConfig(agentId)
      if (response.data.success) {
        await fetchAgents() // 重新获取列表
        return { success: true }
      } else {
        return { success: false, error: response.data.message }
      }
    } catch (error) {
      return { success: false, error: error.message }
    }
  }
  
  // === 工具函数 ===
  
  const updateStats = () => {
    stats.value.total = agents.value.length
    stats.value.active = agents.value.filter(a => a.status === 'active').length
    stats.value.inactive = agents.value.filter(a => a.status === 'inactive').length
    stats.value.partial = agents.value.filter(a => a.status === 'partial').length
    stats.value.error = agents.value.filter(a => a.status === 'error').length
    stats.value.totalServices = agents.value.reduce((sum, a) => sum + (a.services || 0), 0)
    stats.value.totalTools = agents.value.reduce((sum, a) => sum + (a.tools || 0), 0)
  }
  
  const setCurrentAgent = (agent) => {
    currentAgent.value = agent
  }
  
  const getAgentById = (id) => {
    return agents.value.find(a => a.id === id)
  }
  
  const searchAgents = (query) => {
    if (!query) return agents.value
    
    const lowerQuery = query.toLowerCase()
    return agents.value.filter(agent => 
      agent.name.toLowerCase().includes(lowerQuery) ||
      agent.id.toLowerCase().includes(lowerQuery) ||
      (agent.description && agent.description.toLowerCase().includes(lowerQuery))
    )
  }
  
  const resetStore = () => {
    agents.value = []
    currentAgent.value = null
    stats.value = {
      total: 0,
      active: 0,
      inactive: 0,
      partial: 0,
      error: 0,
      totalServices: 0,
      totalTools: 0
    }
    lastUpdateTime.value = null
  }
  
  return {
    // 状态
    agents,
    currentAgent,
    loading,
    lastUpdateTime,
    stats,
    
    // 计算属性
    agentsByStatus,
    activeAgents,
    inactiveAgents,
    partialAgents,
    errorAgents,
    
    // 方法
    fetchAgents,
    getAgentServices,
    getAgentTools,
    getAgentStats,
    addService,
    deleteService,
    updateService,
    restartService,
    useTool,
    checkServices,
    resetAgentConfig,
    updateStats,
    setCurrentAgent,
    getAgentById,
    searchAgents,
    resetStore
  }
})
