import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '@/api'
import { useAppStore } from './app'
import { SERVICE_LIFECYCLE_STATES } from '@/api/config'

export const useServicesStore = defineStore('services', () => {
  const appStore = useAppStore()

  // 状态
  const services = ref([])
  const currentService = ref(null)
  const loading = ref(false)
  const lastUpdateTime = ref(null)

  // 服务统计
  const stats = ref({
    total: 0,
    running: 0,
    stopped: 0,
    error: 0,
    local: 0,
    remote: 0
  })

  // 新增状态
  const serviceHealth = ref({}) // service_id -> health_info
  const connectionStatus = ref({}) // service_id -> connection_status
  const serviceMetrics = ref({}) // service_id -> metrics
  const errors = ref([])
  const lastError = ref(null)

  // 详细加载状态
  const loadingStates = ref({
    services: false,
    health: false,
    adding: false,
    removing: false,
    updating: false,
    checking: false
  })

  // 服务配置
  const serviceConfig = ref({
    autoRefresh: false, // 暂时禁用自动刷新
    refreshInterval: 60000, // 增加到60秒
    healthCheckInterval: 120000, // 增加到2分钟
    maxRetries: 2, // 减少重试次数
    timeout: 15000 // 增加超时时间
  })
  
  // 计算属性
  const servicesByStatus = computed(() => {
    return services.value.reduce((acc, service) => {
      const status = service.status || 'unknown'
      if (!acc[status]) acc[status] = []
      acc[status].push(service)
      return acc
    }, {})
  })
  
  const runningServices = computed(() => {
    return services.value.filter(s => s.status === 'healthy' || s.status === 'running')
  })
  
  const localServices = computed(() => {
    return services.value.filter(s => s.command)
  })
  
  const remoteServices = computed(() => {
    return services.value.filter(s => s.url)
  })
  
  const healthyServices = computed(() => {
    return services.value.filter(s => s.status === 'healthy')
  })
  
  const unhealthyServices = computed(() => {
    return services.value.filter(s => s.status !== 'healthy')
  })

  // 新增计算属性
  const failedServices = computed(() => {
    return services.value.filter(s => s.status === 'error' || s.status === 'unhealthy')
  })

  const unknownServices = computed(() => {
    return services.value.filter(s => !s.status || s.status === 'unknown')
  })

  // 是否有任何加载状态
  const isLoading = computed(() => {
    return Object.values(loadingStates.value).some(Boolean) || loading.value
  })

  // 是否有错误
  const hasErrors = computed(() => {
    return errors.value.length > 0
  })

  // 最近的错误
  const recentErrors = computed(() => {
    return errors.value.slice(-5).reverse()
  })

  // 活跃的服务（已连接且健康）
  const activeServices = computed(() => {
    return services.value.filter(service => {
      const health = serviceHealth.value[service.name]
      const connection = connectionStatus.value[service.name]
      return service.status === 'healthy' && (!connection || connection.connected !== false)
    })
  })

  // 服务健康率
  const healthRate = computed(() => {
    const total = services.value.length
    const healthy = healthyServices.value.length
    return total > 0 ? (healthy / total * 100).toFixed(1) : 0
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
      type: error.type || 'service-error',
      source: error.source || 'services-store'
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

    loading.value = true
    setLoadingState('services', true)

    try {
      appStore?.setLoadingState('services', true)

      const response = await api.store.listServices()

      // 🔍 调试：检查API返回的数据格式
      console.log('🔍 [DEBUG] API返回的原始数据:', response)
      console.log('🔍 [DEBUG] response.data类型:', typeof response.data)
      console.log('🔍 [DEBUG] response.data是否为数组:', Array.isArray(response.data))

      // 🔧 改进：处理新的API响应格式和数据结构
      let rawServices = []

      console.log('🔍 [DEBUG] 完整API响应:', response)
      console.log('🔍 [DEBUG] response.data:', response.data)

      // 处理不同的响应格式
      if (response.data && response.data.success && response.data.data && response.data.data.services) {
        // 新格式：{ success: true, data: { services: [...], total_services: 2 } }
        rawServices = response.data.data.services
        console.log('✅ [DEBUG] 使用新格式 response.data.data.services')
      } else if (response.data && response.data.success && Array.isArray(response.data.data)) {
        // 兼容旧格式：data直接是数组
        rawServices = response.data.data
        console.log('✅ [DEBUG] 使用旧格式 response.data.data (数组)')
      } else if (Array.isArray(response.data)) {
        rawServices = response.data
        console.log('✅ [DEBUG] 使用 response.data (直接数组)')
      } else if (Array.isArray(response)) {
        rawServices = response
        console.log('✅ [DEBUG] 使用 response (直接数组)')
      } else if (response.data && Array.isArray(response.data.services)) {
        rawServices = response.data.services
        console.log('✅ [DEBUG] 使用 response.data.services')
      } else {
        console.warn('⚠️ API返回的数据格式不正确，使用空数组')
        console.warn('实际响应结构:', {
          hasData: !!response.data,
          hasSuccess: !!(response.data && response.data.success),
          hasDataData: !!(response.data && response.data.data),
          hasServices: !!(response.data && response.data.data && response.data.data.services),
          dataType: typeof response.data,
          dataDataType: response.data && typeof response.data.data
        })
        rawServices = []
      }

      console.log('🔍 [DEBUG] 提取的rawServices:', rawServices)
      console.log('🔍 [DEBUG] rawServices长度:', rawServices.length)

      // 🔧 处理新的数据结构，确保所有服务都有必要的字段
      services.value = rawServices.map(service => ({
        ...service,
        // 确保激活状态字段存在
        is_active: service.is_active !== undefined ? service.is_active : (service.state_metadata !== null),
        // 确保生命周期字段存在
        consecutive_successes: service.consecutive_successes || 0,
        consecutive_failures: service.consecutive_failures || 0,
        last_ping_time: service.last_ping_time || null,
        error_message: service.error_message || null,
        reconnect_attempts: service.reconnect_attempts || 0,
        state_entered_time: service.state_entered_time || null,
        // 添加UI状态字段
        activating: false,
        restarting: false
      }))

      // 统计激活和配置服务数量
      const activeServices = services.value.filter(s => s.is_active).length
      const configOnlyServices = services.value.length - activeServices

      console.log(`✅ [Store] 成功获取 ${services.value.length} 个服务 (已激活: ${activeServices}, 仅配置: ${configOnlyServices})`)
      console.log('🔍 [DEBUG] 处理后的services.value:', services.value)

      updateStats()
      lastUpdateTime.value = new Date()

      console.log(`📋 Loaded ${services.value.length} services`)
      return services.value
    } catch (error) {
      console.error('获取服务列表失败:', error)
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
  
  const addService = async (serviceData) => {
    try {
      setLoadingState('adding', true)
      appStore?.setLoadingState('services', true)

      const response = await api.store.addService(serviceData)
      if (response.data.success) {
        await fetchServices(true) // 强制重新获取列表

        appStore?.addNotification({
          title: '服务添加成功',
          message: `服务 "${serviceData.name || serviceData.command}" 已成功添加`,
          type: 'success'
        })

        return { success: true, data: response.data }
      } else {
        const errorMsg = response.data.message || '添加服务失败'
        addError({
          message: errorMsg,
          type: 'add-error',
          source: 'addService'
        })
        return { success: false, error: errorMsg }
      }
    } catch (error) {
      const errorMsg = error.message || '添加服务失败'
      addError({
        message: errorMsg,
        type: 'add-error',
        source: 'addService'
      })
      return { success: false, error: errorMsg }
    } finally {
      setLoadingState('adding', false)
      appStore?.setLoadingState('services', false)
    }
  }
  
  const deleteService = async (serviceName) => {
    try {
      setLoadingState('removing', true)
      appStore?.setLoadingState('services', true)

      const response = await api.store.deleteService(serviceName)
      if (response.data.success) {
        // 从本地状态中移除
        const index = services.value.findIndex(s => s.name === serviceName)
        if (index > -1) {
          services.value.splice(index, 1)

          // 清理相关状态
          delete serviceHealth.value[serviceName]
          delete connectionStatus.value[serviceName]
          delete serviceMetrics.value[serviceName]
        }

        updateStats()

        appStore?.addNotification({
          title: '服务移除成功',
          message: `服务 "${serviceName}" 已成功移除`,
          type: 'success'
        })

        return { success: true }
      } else {
        const errorMsg = response.data.message || '删除服务失败'
        addError({
          message: errorMsg,
          type: 'delete-error',
          source: 'deleteService'
        })
        return { success: false, error: errorMsg }
      }
    } catch (error) {
      const errorMsg = error.message || '删除服务失败'
      addError({
        message: errorMsg,
        type: 'delete-error',
        source: 'deleteService'
      })
      return { success: false, error: errorMsg }
    } finally {
      setLoadingState('removing', false)
      appStore?.setLoadingState('services', false)
    }
  }
  
  const restartService = async (serviceName) => {
    try {
      const response = await api.store.restartService(serviceName)
      if (response.data.success) {
        await fetchServices()
        return { success: true }
      } else {
        return { success: false, error: response.data.message }
      }
    } catch (error) {
      return { success: false, error: error.message }
    }
  }
  
  const updateService = async (serviceName, config) => {
    try {
      const response = await api.store.updateService(serviceName, config)
      if (response.data.success) {
        await fetchServices()
        return { success: true }
      } else {
        return { success: false, error: response.data.message }
      }
    } catch (error) {
      return { success: false, error: error.message }
    }
  }
  
  const batchUpdateServices = async (updates) => {
    try {
      const response = await api.store.batchUpdateServices(updates)
      if (response.data.success) {
        await fetchServices()
        return { success: true }
      } else {
        return { success: false, error: response.data.message }
      }
    } catch (error) {
      return { success: false, error: error.message }
    }
  }
  
  const batchDeleteServices = async (serviceNames) => {
    try {
      const response = await api.store.batchDeleteServices(serviceNames)
      if (response.data.success) {
        await fetchServices()
        return { success: true }
      } else {
        return { success: false, error: response.data.message }
      }
    } catch (error) {
      return { success: false, error: error.message }
    }
  }
  
  const batchRestartServices = async (serviceNames) => {
    try {
      const response = await api.store.batchRestartServices(serviceNames)
      if (response.data.success) {
        await fetchServices()
        return { success: true }
      } else {
        return { success: false, error: response.data.message }
      }
    } catch (error) {
      return { success: false, error: error.message }
    }
  }
  
  const checkServicesHealth = async () => {
    try {
      setLoadingState('checking', true)

      const response = await api.store.checkServices()
      // 更新服务状态
      if (response.data && Array.isArray(response.data)) {
        response.data.forEach(healthInfo => {
          const service = services.value.find(s => s.name === healthInfo.name)
          if (service) {
            service.status = healthInfo.status
            service.last_heartbeat = healthInfo.last_heartbeat

            // 更新健康状态
            updateServiceHealth(healthInfo.name, {
              status: healthInfo.status,
              lastCheck: Date.now(),
              details: healthInfo
            })
          }
        })
        updateStats()
      }
      return response.data
    } catch (error) {
      console.error('健康检查失败:', error)
      addError({
        message: `健康检查失败: ${error.message}`,
        type: 'health-check-error',
        source: 'checkServicesHealth'
      })
      throw error
    } finally {
      setLoadingState('checking', false)
    }
  }

  // 更新服务健康状态
  const updateServiceHealth = (serviceName, health) => {
    serviceHealth.value[serviceName] = {
      ...health,
      lastCheck: Date.now()
    }
  }

  // 更新服务连接状态
  const updateConnectionStatus = (serviceName, status) => {
    connectionStatus.value[serviceName] = {
      ...status,
      lastUpdate: Date.now()
    }
  }

  // 获取系统资源信息
  const fetchSystemResources = async () => {
    try {
      const response = await api.monitoring.getSystemResources()

      if (response.success && response.data) {
        return response.data
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
    }
  }

  // 刷新所有数据
  const refreshAll = async () => {
    try {
      setLoadingState('updating', true)

      await Promise.all([
        fetchServices(true),
        checkServicesHealth(),
        fetchSystemResources()
      ])

      lastUpdateTime.value = new Date()

      appStore?.addNotification({
        title: '数据刷新完成',
        message: '所有服务数据已更新',
        type: 'success'
      })

    } catch (error) {
      console.error('Failed to refresh all data:', error)
      addError({
        message: `刷新数据失败: ${error.message}`,
        type: 'refresh-error',
        source: 'refreshAll'
      })
    } finally {
      setLoadingState('updating', false)
    }
  }
  
  const updateStats = () => {
    // 安全检查：确保services.value是数组
    if (!Array.isArray(services.value)) {
      console.warn('⚠️ updateStats: services.value不是数组，跳过统计更新')
      return
    }

    stats.value.total = services.value.length
    stats.value.running = services.value.filter(s => s.status === 'healthy' || s.status === 'running').length
    stats.value.stopped = services.value.filter(s => s.status === 'stopped').length
    stats.value.error = services.value.filter(s => s.status === 'error' || s.status === 'unhealthy').length
    stats.value.local = services.value.filter(s => s.command).length
    stats.value.remote = services.value.filter(s => s.url).length
  }
  
  const setCurrentService = (service) => {
    currentService.value = service
  }
  
  const getServiceByName = (name) => {
    return services.value.find(s => s.name === name)
  }
  
  const resetStore = () => {
    services.value = []
    currentService.value = null
    stats.value = {
      total: 0,
      running: 0,
      stopped: 0,
      error: 0,
      local: 0,
      remote: 0
    }
    lastUpdateTime.value = null

    // 重置新增状态
    serviceHealth.value = {}
    connectionStatus.value = {}
    serviceMetrics.value = {}
    errors.value = []
    lastError.value = null

    // 重置加载状态
    Object.keys(loadingStates.value).forEach(key => {
      loadingStates.value[key] = false
    })
    loading.value = false

    console.log('🔄 Services store reset')
  }
  
  return {
    // 原有状态
    services,
    currentService,
    loading,
    lastUpdateTime,
    stats,

    // 新增状态
    serviceHealth,
    connectionStatus,
    serviceMetrics,
    errors,
    lastError,
    loadingStates,
    serviceConfig,

    // 原有计算属性
    servicesByStatus,
    runningServices,
    localServices,
    remoteServices,
    healthyServices,
    unhealthyServices,

    // 新增计算属性
    failedServices,
    unknownServices,
    isLoading,
    hasErrors,
    recentErrors,
    activeServices,
    healthRate,

    // 原有方法
    fetchServices,
    addService,
    deleteService,
    restartService,
    updateService,
    batchUpdateServices,
    batchDeleteServices,
    batchRestartServices,
    checkServicesHealth,
    updateStats,
    setCurrentService,
    getServiceByName,
    resetStore,

    // 新增方法
    setLoadingState,
    addError,
    clearErrors,
    updateServiceHealth,
    updateConnectionStatus,
    fetchSystemResources,
    refreshAll
  }
})
