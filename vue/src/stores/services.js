import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '@/api'
import { useAppStore } from './app'
import { useErrorHandler, useLoadingState, LOADING_KEYS } from '@/composables'
import { logger } from '@/utils/logger'

export const useServicesStore = defineStore('services', () => {
  const appStore = useAppStore()

  // 使用 composables
  const errorHandler = useErrorHandler({ source: 'services-store' })
  const loadingState = useLoadingState({
    services: false,
    health: false,
    adding: false,
    removing: false,
    updating: false,
    checking: false
  })

  // 状态
  const services = ref([])
  const currentService = ref(null)
  const loading = ref(false)
  const lastUpdateTime = ref(null)

  // 服务统计
  const stats = ref({
    total: 0,
    healthy: 0,
    ready: 0,
    degraded: 0,
    half_open: 0,
    circuit_open: 0,
    disconnected: 0,
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
    return services.value.filter(s => s.status === 'healthy' || s.status === 'ready')
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
    return services.value.filter(s => !['healthy', 'ready'].includes(s.status))
  })

  // 新增计算属性
  const failedServices = computed(() => {
    return services.value.filter(s => ['circuit_open', 'half_open', 'disconnected', 'degraded'].includes(s.status))
  })

  const unknownServices = computed(() => {
    return services.value.filter(s => !s.status || s.status === 'unknown')
  })

  // 是否有任何加载状态
  const isLoading = computed(() => {
    return loadingState.isLoading.value || loading.value
  })

  // 是否有错误（使用 composable）
  const hasErrors = computed(() => errorHandler.hasErrors.value)

  // 最近的错误（使用 composable）
  const recentErrors = computed(() => errorHandler.recentErrors.value)

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
  
  // 新增方法（使用 composables）
  const setLoadingState = (type, status) => {
    loadingState.setLoading(type, status)
  }

  const addError = (error) => {
    const errorObj = errorHandler.addError(error)
    
    // 同时添加到应用级错误
    if (appStore) {
      appStore.addError(errorObj)
    }
    
    return errorObj
  }

  const clearErrors = () => {
    errorHandler.clearErrors()
  }

  // 方法
  const fetchServices = async (force = false) => {
    if ((loading.value || loadingState.getLoading('services')) && !force) return

    loading.value = true
    setLoadingState('services', true)

    try {
      appStore?.setLoadingState('services', true)

      const servicesArr = await api.store.listServices()

      // 处理数据结构，确保必要字段存在
      services.value = (Array.isArray(servicesArr) ? servicesArr : []).map(service => ({
        ...service,
        is_active: service.is_active !== undefined ? service.is_active : (service.state_metadata !== null),
        consecutive_successes: service.consecutive_successes || 0,
        consecutive_failures: service.consecutive_failures || 0,
        last_ping_time: service.last_ping_time || null,
        error_message: service.error_message || null,
        reconnect_attempts: service.reconnect_attempts || 0,
        state_entered_time: service.state_entered_time || null,
        activating: false,
        restarting: false
      }))

      updateStats()
      lastUpdateTime.value = new Date()

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

      const data = await api.store.checkServices()
      // 更新服务状态
      if (Array.isArray(data)) {
        data.forEach(healthInfo => {
          const service = services.value.find(s => s.name === healthInfo.name)
          if (service) {
            service.status = healthInfo.status
            service.last_heartbeat = healthInfo.last_heartbeat
            service.window_error_rate = healthInfo.window_error_rate ?? service.window_error_rate
            service.latency_p95 = healthInfo.latency_p95 ?? service.latency_p95
            service.latency_p99 = healthInfo.latency_p99 ?? service.latency_p99
            service.sample_size = healthInfo.sample_size ?? service.sample_size
            service.retry_in = healthInfo.retry_in ?? service.retry_in
            service.hard_timeout_in = healthInfo.hard_timeout_in ?? service.hard_timeout_in
            service.lease_remaining = healthInfo.lease_remaining ?? service.lease_remaining
            service.next_retry_time = healthInfo.next_retry_time ?? service.next_retry_time
            service.hard_deadline = healthInfo.hard_deadline ?? service.hard_deadline
            service.lease_deadline = healthInfo.lease_deadline ?? service.lease_deadline

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
      return data
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
      const data = await api.store.getSystemResources()
      return data
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
      logger.warn('⚠️ updateStats: services.value不是数组，跳过统计更新')
      return
    }

    const counts = services.value.reduce((acc, service) => {
      const key = service.status || 'unknown'
      acc[key] = (acc[key] || 0) + 1
      return acc
    }, {})

    stats.value.total = services.value.length
    stats.value.healthy = counts.healthy || 0
    stats.value.ready = counts.ready || 0
    stats.value.degraded = counts.degraded || 0
    stats.value.half_open = counts.half_open || 0
    stats.value.circuit_open = counts.circuit_open || 0
    stats.value.disconnected = counts.disconnected || 0
    stats.value.running = stats.value.healthy + stats.value.ready
    stats.value.stopped = counts.stopped || 0
    stats.value.error = counts.error || 0
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
      healthy: 0,
      ready: 0,
      degraded: 0,
      half_open: 0,
      circuit_open: 0,
      disconnected: 0,
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
    errorHandler.clearErrors()

    // 重置加载状态
    loadingState.resetAll()
    loading.value = false

    logger.debug('🔄 Services store reset')
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
    serviceConfig,
    
    // Composable 实例（用于访问错误和加载状态）
    errorHandler,
    loadingState,

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
