// 服务数据管理 Composable
import { ref, computed, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import { mcpApi } from '@/mcp/api'

export function useServiceData() {
  // 响应式数据
  const loading = ref(false)
  const error = ref<string | null>(null)
  const servicesResponse = ref<any>(null)
  const tableData = ref<any[]>([])
  
  // 计算属性
  const totalServices = computed(() => tableData.value.length)
  const healthyCount = computed(() => tableData.value.filter(s => s.health === 'healthy').length)
  const healthPercentage = computed(() => {
    if (totalServices.value === 0) return 0
    return Math.round((healthyCount.value / totalServices.value) * 100) - 100
  })
  const serviceGrowthPercentage = computed(() => {
    // 模拟服务增长百分比，可以根据实际需求计算
    return Math.random() > 0.5 ? Math.floor(Math.random() * 20) : -Math.floor(Math.random() * 10)
  })
  const healthChartData = computed(() => [
    { value: healthyCount.value, name: '健康' },
    { value: totalServices.value - healthyCount.value, name: '不健康' }
  ])
  const serviceTypeData = computed(() => {
    const types = tableData.value.reduce((acc, service) => {
      acc[service.type] = (acc[service.type] || 0) + 1
      return acc
    }, {} as Record<string, number>)
    return Object.values(types)
  })
  const serviceTypeLabels = computed(() => {
    const types = tableData.value.reduce((acc, service) => {
      acc[service.type] = (acc[service.type] || 0) + 1
      return acc
    }, {} as Record<string, number>)
    return Object.keys(types)
  })
  
  // 横幅文案
  const headerSubtitle = computed(() => {
    const total = totalServices.value
    const healthy = healthyCount.value
    const unhealthy = total - healthy
    return `当前共有 ${total} 个服务，其中 ${healthy} 个健康，${unhealthy} 个不健康。`
  })
  
  // 时间格式化
  const formatTimeAgoFromString = (s?: string) => {
    if (!s) return '-'
    const t = new Date(s).getTime()
    if (Number.isNaN(t)) return s
    const now = Date.now()
    const diff = Math.max(0, now - t)
    const minute = 60 * 1000
    const hour = 60 * minute
    const day = 24 * hour
    if (diff < minute) return '刚刚'
    if (diff < hour) return Math.floor(diff / minute) + '分钟前'
    if (diff < day) return Math.floor(diff / hour) + '小时前'
    return Math.floor(diff / day) + '天前'
  }
  
  // 载入服务数据
  const loadServices = async () => {
    loading.value = true
    error.value = null
    
    try {
      const services = await mcpApi.listServices() // data 为数组
      servicesResponse.value = { data: services }

      // 转换到表格行（对齐最新响应结构）
      tableData.value = (services || []).map((s: any, idx: number) => {
        const endpoint = (s.url && String(s.url).trim())
          ? s.url
          : (s.command
              ? [s.command, ...(Array.isArray(s.args) ? s.args : [])].join(' ').trim()
              : '')
        const typeLabel = s.type === 'streamable_http' ? 'HTTP' : (s.type || 'Unknown').toUpperCase()
        const health = s.status || 'unknown'
        const inferredRunStatus = ['healthy', 'reconnecting', 'warning'].includes(health) ? 'running' : 'stopped'
        const lastCheck = s.last_check || '-'

        return {
          id: idx + 1,
          name: s.name,
          type: typeLabel,
          endpoint,
          status: inferredRunStatus,
          // 移除了对外展示的健康列；内部仍保留 health 便于统计
          health,
          lastCheck,
          lastCheckAgo: formatTimeAgoFromString(lastCheck),
          toolCount: s.tools_count ?? s.tool_count ?? 0,
          description: s.url ? '远程服务' : (s.command ? '本地服务' : '服务')
        }
      })
    } catch (err) {
      error.value = err instanceof Error ? err.message : '获取服务列表失败'
      ElMessage.error('获取服务列表失败')
      console.error(err)
    } finally {
      loading.value = false
    }
  }
  
  // 刷新服务数据
  const refreshServices = async () => {
    await loadServices()
    ElMessage.success('服务状态已刷新')
  }
  
  // 服务操作API（TODO: 实现具体的API调用）
  const restartService = async (serviceName: string) => {
    ElMessage.info(`正在重启服务: ${serviceName}`)
    // TODO: 实现具体的重启API调用
    // await serviceApi.restartService(serviceName)
    ElMessage.success(`服务 ${serviceName} 重启成功`)
    await loadServices()
  }
  
  const toggleService = async (serviceName: string, currentStatus: string) => {
    const action = currentStatus === 'running' ? '停止' : '启动'
    ElMessage.info(`正在${action}服务: ${serviceName}`)
    // TODO: 实现具体的启动/停止API调用
    // await serviceApi.toggleService(serviceName, currentStatus === 'running' ? 'stop' : 'start')
    ElMessage.success(`服务已${action}`)
    await loadServices()
  }
  
  const deleteService = async (serviceName: string) => {
    ElMessage.info(`正在删除服务: ${serviceName}`)
    // TODO: 实现具体的删除API调用
    // await serviceApi.deleteService(serviceName)
    ElMessage.success('服务已删除')
    await loadServices()
  }
  
  // 状态映射
  const getStatusType = (status: string) => {
    const statusMap: Record<string, string> = {
      healthy: 'success',
      warning: 'warning',
      reconnecting: 'warning',
      unreachable: 'danger',
      disconnected: 'info',
      unknown: 'info',
      running: 'success',
      stopped: 'danger',
      starting: 'warning',
      error: 'danger'
    }
    return statusMap[status] || 'info'
  }

  const getStatusText = (status: string) => {
    const statusMap: Record<string, string> = {
      healthy: '健康',
      warning: '警告',
      reconnecting: '重连中',
      unreachable: '不可达',
      disconnected: '已断开',
      unknown: '未知',
      running: '运行中',
      stopped: '已停止',
      starting: '启动中',
      error: '错误'
    }
    return statusMap[status] || status
  }
  
  // 生命周期
  onMounted(() => {
    loadServices()
  })
  
  return {
    // 状态
    loading,
    error,
    tableData,
    servicesResponse,
    
    // 计算属性
    totalServices,
    healthyCount,
    healthPercentage,
    serviceGrowthPercentage,
    healthChartData,
    serviceTypeData,
    serviceTypeLabels,
    headerSubtitle,
    
    // 方法
    loadServices,
    refreshServices,
    restartService,
    toggleService,
    deleteService,
    getStatusType,
    getStatusText,
    formatTimeAgoFromString
  }
}
