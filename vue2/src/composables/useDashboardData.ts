// 仪表盘数据管理 Composable
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { ElMessage } from 'element-plus'
import { dashboardApi, transformDashboardData, type ToolRecord, type SystemResources, type AgentSummary } from '@/mcp/api/dashboard'

export function useDashboardData() {
  // 响应式数据
  const loading = ref(false)
  const error = ref<string | null>(null)
  
  // 原始数据（直接存放 MCP API 原始响应，保持灵活）
  const services = ref<any>(null)            // 期望形如 { success, data: { services: [...] } }
  const toolsData = ref<any>(null)           // 期望形如 { success, data: [...], metadata: {...} }
  const toolRecords = ref<any>(null)         // 期望形如 { success, data: { executions: [...] } }
  const systemResources = ref<SystemResources | null>(null)
  const agentsSummary = ref<AgentSummary | null>(null)
  const healthSummary = ref<any>(null)
  
  // 自动刷新定时器
  let refreshTimer: NodeJS.Timeout | null = null
  
  // 计算属性 - 统计卡片数据
  const statCards = computed(() => {
    const serviceStats = transformDashboardData.transformServices(services.value)
    const toolStats = transformDashboardData.transformTools(toolsData.value)
    const agentStats = agentsSummary.value

    return [
      {
        des: '服务总数',
        icon: '&#xe721;',
        num: serviceStats.total,
        changeText: '健康服务',
        change: serviceStats.healthy.toString(),
        changeClass: serviceStats.healthy > 0 ? 'text-success' : 'text-warning',
        route: '/services'
      },
      {
        des: '工具总数',
        icon: '&#xe7aa;',
        num: toolStats.total,
        changeText: '可执行工具',
        change: toolStats.executable.toString(),
        changeClass: toolStats.executable > 0 ? 'text-success' : 'text-warning',
        route: '/tools'
      },
      {
        des: 'Agent 数量',
        icon: '&#xe82a;',
        num: agentStats?.total_agents || 0,
        changeText: '活跃Agent',
        change: (agentStats?.active_agents || 0).toString(),
        changeClass: (agentStats?.active_agents || 0) > 0 ? 'text-success' : 'text-info',
        route: '/agents'
      },
      {
        des: '今日调用',
        icon: '&#xe724;',
        num: toolRecords.value?.data?.executions?.length || 0,
        changeText: '成功率',
        change: (() => {
          const executions = toolRecords.value?.data?.executions || []
          if (executions.length === 0) return '0%'
          const successCount = executions.filter((r: any) => !r.error).length
          return Math.round((successCount / executions.length) * 100) + '%'
        })(),
        changeClass: (toolRecords.value?.data?.executions?.length || 0) > 0 ? 'text-success' : 'text-info',
        route: '/tools/records'
      }
    ]
  })
  
  
  // 计算属性 - 24小时折线图数据
  const hourlyChartData = computed(() => {
    const executions = toolRecords.value?.data?.executions || []
    return transformDashboardData.transformToolRecordsToChart(executions, '24h')
  })
  
  // 计算属性 - 24小时图表标签
  const hourlyLabels = computed(() => {
    const labels: string[] = []
    const now = new Date()
    for (let i = 23; i >= 0; i--) {
      const time = new Date(now.getTime() - i * 60 * 60 * 1000)
      labels.push(`${String(time.getHours()).padStart(2, '0')}:00`)
    }
    return labels
  })
  
  // 计算属性 - 30天折线图数据
  const monthlyChartData = computed(() => {
    const executions = toolRecords.value?.data?.executions || []
    return transformDashboardData.transformToolRecordsToChart(executions, '30d')
  })
  
  // 计算属性 - 30天图表标签
  const monthlyLabels = computed(() => {
    const labels: string[] = []
    const now = new Date()
    for (let i = 29; i >= 0; i--) {
      const date = new Date(now.getTime() - i * 24 * 60 * 60 * 1000)
      labels.push(`${date.getMonth() + 1}/${date.getDate()}`)
    }
    return labels
  })

  // 服务列表（便于统计）
  const servicesList = computed(() => {
    return services.value?.data?.services || []
  })

  // 顶部环图：服务健康 vs 不健康
  const serviceHealthRingData = computed(() => {
    const total = servicesList.value.length
    if (total === 0) return [{ value: 1, name: '暂无数据' }]
    const healthy = servicesList.value.filter((s: any) => s.status === 'healthy').length
    const unhealthy = total - healthy
    return [
      { value: healthy, name: '健康' },
      { value: unhealthy, name: '不健康' }
    ]
  })
  const healthyServiceCount = computed(() => servicesList.value.filter((s: any) => s.status === 'healthy').length)

  // 服务传输类型统计
  const serviceTransportStats = computed(() => {
    const transportCounts = servicesList.value.reduce((acc: any, service: any) => {
      const transport = service.transport || 'unknown'
      acc[transport] = (acc[transport] || 0) + 1
      return acc
    }, {})
    
    return Object.entries(transportCounts).map(([name, value]) => ({
      name: name === 'stdio' ? 'Stdio' : name === 'streamable_http' ? 'HTTP' : name,
      value: value as number
    }))
  })
  
  // 服务传输类型柱状图数据（用于横向柱状图）
  const serviceTransportBarData = computed(() => {
    return serviceTransportStats.value.map(item => item.value)
  })
  
  // 服务传输类型标签
  const serviceTransportLabels = computed(() => {
    return serviceTransportStats.value.map(item => item.name)
  })

  // 近7日柱图：每日调用次数
  const weeklyBarData = computed(() => {
    const counts = new Array(7).fill(0)
    const now = new Date()
    const executions = toolRecords.value?.data?.executions || []
    executions.forEach((r: any) => {
      const t = new Date(r.execution_time)
      const diffDays = Math.floor((+now - +t) / (24 * 60 * 60 * 1000))
      if (diffDays >= 0 && diffDays < 7) counts[6 - diffDays]++
    })
    return counts
  })
  const weeklyLabels = computed(() => {
    const labels: string[] = []
    const now = new Date()
    for (let i = 6; i >= 0; i--) {
      const d = new Date(now.getTime() - i * 24 * 60 * 60 * 1000)
      const mm = String(d.getMonth() + 1).padStart(2, '0')
      const dd = String(d.getDate()).padStart(2, '0')
      labels.push(`${mm}-${dd}`)
    }
    return labels
  })
  const weeklyTotalCalls = computed(() => weeklyBarData.value.reduce((a, b) => a + b, 0))

  // 计算属性 - 系统状态
  const systemStatus = computed(() => {
    if (!systemResources.value) {
      return { running: false, uptime: '未知' }
    }
    
    return {
      running: true,
      uptime: systemResources.value.server_uptime
    }
  })
  
  // 计算属性 - 系统信息
  const systemInfo = computed(() => {
    if (!systemResources.value) {
      return { uptime: '未知', memory_usage: 0, disk_usage: 0 }
    }
    
    return transformDashboardData.transformSystemResources(systemResources.value)
  })
  
  // 快速操作数据
  const quickActions = ref([
    {
      id: 1,
      title: '服务管理',
      description: '管理和配置MCP服务',
      icon: '&#xe721;',
      iconBgColor: '#409eff',
      route: '/services',
      action: null
    },
    {
      id: 2,
      title: '工具执行',
      description: '执行和测试MCP工具',
      icon: '&#xe7aa;',
      iconBgColor: '#67c23a',
      route: '/tools',
      action: null
    },
    {
      id: 3,
      title: 'Agent管理',
      description: '管理智能体配置',
      icon: '&#xe82a;',
      iconBgColor: '#e6a23c',
      route: '/agents',
      action: null
    },
    {
      id: 4,
      title: '刷新数据',
      description: '重新加载仪表盘数据',
      icon: '&#xe724;',
      iconBgColor: '#f56c6c',
      route: null,
      action: 'refresh'
    }
  ])
  
  // 获取所有数据
  const fetchAllData = async () => {
    loading.value = true
    error.value = null
    
    try {
      // 并行获取所有数据
      const [
        servicesRes,
        toolsRes,
        recordsRes,
        resourcesRes,
        agentsRes,
        healthRes
      ] = await Promise.allSettled([
        dashboardApi.getServices(),
        dashboardApi.getTools(),
        dashboardApi.getToolRecords(50),
        dashboardApi.getSystemResources(),
        dashboardApi.getAgentsSummary(),
        dashboardApi.getHealthSummary()
      ])
      
      // 处理服务数据
      if (servicesRes.status === 'fulfilled') {
        console.log('Services API Response:', servicesRes.value)
        services.value = servicesRes.value
      } else {
        console.error('Services API Error:', servicesRes.reason)
      }

      // 处理工具数据
      if (toolsRes.status === 'fulfilled') {
        console.log('Tools API Response:', toolsRes.value)
        toolsData.value = toolsRes.value
      } else {
        console.error('Tools API Error:', toolsRes.reason)
      }

      // 处理工具记录
      if (recordsRes.status === 'fulfilled') {
        console.log('Tool Records API Response:', recordsRes.value)
        // 保存完整的响应对象，确保结构：{ success, data: { executions: [...], summary: {...} } }
        toolRecords.value = recordsRes.value
      } else {
        console.error('Tool Records API Error:', recordsRes.reason)
      }

      // 处理系统资源
      if (resourcesRes.status === 'fulfilled') {
        console.log('System Resources API Response:', resourcesRes.value)
        systemResources.value = resourcesRes.value?.data
      } else {
        console.error('System Resources API Error:', resourcesRes.reason)
      }

      // 处理 Agent 统计
      if (agentsRes.status === 'fulfilled') {
        console.log('Agents Summary API Response:', agentsRes.value)
        agentsSummary.value = agentsRes.value?.data
      } else {
        console.error('Agents Summary API Error:', agentsRes.reason)
      }

      // 处理健康状态
      if (healthRes.status === 'fulfilled') {
        console.log('Health Summary API Response:', healthRes.value)
        healthSummary.value = healthRes.value?.data
      } else {
        console.error('Health Summary API Error:', healthRes.reason)
      }
      
    } catch (err) {
      error.value = err instanceof Error ? err.message : '获取数据失败'
      ElMessage.error('仪表盘数据加载失败')
    } finally {
      loading.value = false
    }
  }
  
  // 刷新数据
  const refreshData = async () => {
    await fetchAllData()
    ElMessage.success('数据刷新成功')
  }
  
  // 启动自动刷新
  const startAutoRefresh = (interval = 30000) => {
    if (refreshTimer) {
      clearInterval(refreshTimer)
    }
    refreshTimer = setInterval(fetchAllData, interval)
  }
  
  // 停止自动刷新
  const stopAutoRefresh = () => {
    if (refreshTimer) {
      clearInterval(refreshTimer)
      refreshTimer = null
    }
  }
  
  // 生命周期
  onMounted(() => {
    fetchAllData()
    startAutoRefresh()
  })
  
  onUnmounted(() => {
    stopAutoRefresh()
  })
  
  return {
    // 状态
    loading,
    error,
    
    // 原始数据
    services,
    toolsData,
    toolRecords,
    systemResources,
    agentsSummary,
    healthSummary,
    
    // 计算属性
    statCards,
    serviceHealthRingData,
    healthyServiceCount,
    serviceTransportStats,
    serviceTransportBarData,
    serviceTransportLabels,
    weeklyBarData,
    weeklyLabels,
    hourlyChartData,
    hourlyLabels,
    monthlyChartData,
    monthlyLabels,
    systemStatus,
    systemInfo,
    quickActions,
    
    // 方法
    fetchAllData,
    refreshData,
    startAutoRefresh,
    stopAutoRefresh
  }
}
