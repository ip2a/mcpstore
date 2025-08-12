<template>
  <div class="dashboard">
    <!-- 错误状态 -->
    <ErrorState
      v-if="hasError"
      :type="errorType"
      :title="errorTitle"
      :description="errorDescription"
      :show-details="showErrorDetails"
      :error-details="errorDetails"
      @retry="handleRetry"
    />

    <!-- 正常内容 -->
    <div v-else>
      <!-- 第一行：紧凑的状态卡片和快捷操作 -->
      <el-row :gutter="16">
      <!-- 系统状态卡片 -->
      <el-col :span="4">
        <el-card class="status-card compact-card">
          <div class="card-header">
            <el-icon><Monitor /></el-icon>
            <span>系统状态</span>
          </div>
          <div class="card-content">
            <div class="status-item">
              <span class="label">运行状态</span>
              <el-tag :type="systemStatus.running ? 'success' : 'danger'" size="small">
                {{ systemStatus.running ? '运行中' : '已停止' }}
              </el-tag>
            </div>
            <div class="status-item">
              <span class="label">运行时间</span>
              <span class="value">{{ systemInfo.uptime }}</span>
            </div>
          </div>
        </el-card>
      </el-col>

      <!-- 快速操作 - 2行2列布局 -->
      <el-col :span="6">
        <el-card class="status-card compact-card">
          <div class="card-header">
            <el-icon><Operation /></el-icon>
            <span>快速操作</span>
          </div>
          <div class="quick-actions-grid">
            <el-button size="small" type="primary" @click="$router.push('/services/add')">
              <el-icon><Plus /></el-icon>
              添加服务
            </el-button>
            <el-button size="small" type="success" @click="$router.push('/tools/execute')">
              <el-icon><VideoPlay /></el-icon>
              执行工具
            </el-button>
            <el-button size="small" type="info" @click="$router.push('/agents/create')">
              <el-icon><UserFilled /></el-icon>
              创建Agent
            </el-button>
            <el-button size="small" type="warning" @click="refreshData">
              <el-icon><Refresh /></el-icon>
              刷新数据
            </el-button>
          </div>
        </el-card>
      </el-col>

      <!-- 工具统计 -->
      <el-col :span="4">
        <el-card class="status-card compact-card">
          <div class="card-header">
            <el-icon><Tools /></el-icon>
            <span>工具统计</span>
          </div>
          <div class="card-content">
            <div class="status-item">
              <span class="label">可用工具</span>
              <span class="value">{{ toolStats.available }}</span>
            </div>
            <div class="status-item">
              <span class="label">今日调用</span>
              <span class="value">{{ toolStats.todayCalls }}</span>
            </div>
          </div>
        </el-card>
      </el-col>

      <!-- Agent统计 -->
      <el-col :span="4">
        <el-card class="status-card compact-card">
          <div class="card-header">
            <el-icon><User /></el-icon>
            <span>Agent统计</span>
          </div>
          <div class="card-content">
            <div class="status-item">
              <span class="label">活跃Agent</span>
              <span class="value">{{ agentStats.active }}</span>
            </div>
            <div class="status-item">
              <span class="label">总Agent数</span>
              <span class="value">{{ agentStats.total }}</span>
            </div>
          </div>
        </el-card>
      </el-col>

      <!-- 服务统计 - 扩展版 -->
      <el-col :span="6">
        <el-card class="status-card compact-card">
          <div class="card-header">
            <el-icon><Connection /></el-icon>
            <span>服务统计</span>
          </div>
          <div class="service-stats-grid">
            <div class="stat-item">
              <div class="stat-label">总服务数</div>
              <div class="stat-value">{{ serviceStats.total }}</div>
            </div>
            <div class="stat-item">
              <div class="stat-label">远程服务</div>
              <div class="stat-value text-primary">{{ serviceStats.remote }}</div>
            </div>
            <div class="stat-item">
              <div class="stat-label">本地服务</div>
              <div class="stat-value text-info">{{ serviceStats.local }}</div>
            </div>
            <div class="stat-item">
              <div class="stat-label">健康服务</div>
              <div class="stat-value text-success">{{ serviceStats.healthy }}</div>
            </div>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 第二行：工具日志、健康服务、今日趋势 -->
    <el-row :gutter="16" style="margin-top: 16px;">
      <!-- 工具使用日志 -->
      <el-col :span="8">
        <el-card v-loading="toolStatsLoading" element-loading-text="" element-loading-spinner="el-icon-loading" element-loading-background="rgba(255, 255, 255, 0.5)" class="logs-card">
          <template #header>
            <div class="card-header">
              <el-icon><Tools /></el-icon>
              <span>工具使用日志</span>
              <el-button
                size="small"
                :icon="Refresh"
                @click="refreshToolStats"
                :loading="toolStatsLoading"
              >
                刷新
              </el-button>
            </div>
          </template>
          <div class="tool-logs-container">
            <div class="tool-logs-list">
              <div
                v-for="tool in topTools"
                :key="tool.tool_name"
                class="tool-log-item"
              >
                <div class="tool-log-header">
                  <div class="tool-name">{{ tool.tool_name }}</div>
                  <div class="tool-time">{{ formatLastExecuted(tool.last_executed) }}</div>
                </div>
                <div class="tool-log-details">
                  <span class="service-tag">{{ tool.service_name }}</span>
                  <span class="execution-count">{{ tool.execution_count }}次</span>
                  <span class="success-rate" :class="getSuccessRateClass(tool.success_rate)">
                    {{ tool.success_rate.toFixed(1) }}%
                  </span>
                  <span class="response-time">{{ tool.average_response_time.toFixed(0) }}ms</span>
                </div>
              </div>
            </div>
            <div v-if="topTools.length === 0" class="empty-logs">
              <el-icon><Tools /></el-icon>
              <span>暂无工具使用记录</span>
            </div>
          </div>
        </el-card>
      </el-col>

      <!-- 健康服务状态 -->
      <el-col :span="8">
        <el-card v-loading="servicesLoading" element-loading-text="" element-loading-spinner="el-icon-loading" element-loading-background="rgba(255, 255, 255, 0.5)" class="services-card">
          <template #header>
            <div class="card-header">
              <el-icon><CircleCheck /></el-icon>
              <span>健康服务</span>
              <el-button
                size="small"
                :icon="Refresh"
                @click="refreshHealthyServices"
                :loading="servicesLoading"
              >
                刷新
              </el-button>
            </div>
          </template>
          <div class="healthy-services-container">
            <div class="healthy-services-list">
              <div
                v-for="service in healthyServices"
                :key="service.name"
                class="service-item"
              >
                <div class="service-status">
                  <el-icon class="status-icon healthy"><CircleCheck /></el-icon>
                </div>
                <div class="service-info">
                  <div class="service-name">{{ service.name }}</div>
                  <div class="service-type">{{ getServiceType(service) }}</div>
                </div>
                <div class="service-tools">
                  <el-tag size="small" type="info">{{ service.toolCount || 0 }} 工具</el-tag>
                </div>
              </div>
            </div>
            <div v-if="healthyServices.length === 0" class="empty-services">
              <el-icon><Warning /></el-icon>
              <span>暂无健康服务</span>
            </div>
          </div>
        </el-card>
      </el-col>

      <!-- 今日24小时趋势图 -->
      <el-col :span="8">
        <el-card v-loading="todayChartLoading" element-loading-text="" element-loading-spinner="el-icon-loading" element-loading-background="rgba(255, 255, 255, 0.3)" class="chart-card">
          <template #header>
            <div class="card-header">
              <el-icon><TrendCharts /></el-icon>
              <span>今日趋势 (24小时)</span>
              <el-button
                size="small"
                :icon="Refresh"
                @click="refreshTodayChart"
                :loading="todayChartLoading"
              >
                刷新
              </el-button>
            </div>
          </template>
          <div class="chart-container today-chart">
            <div ref="todayChart" class="trend-chart"></div>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 第三行：30天趋势图 -->
    <el-row :gutter="16" style="margin-top: 16px;">
      <el-col :span="24">
        <el-card v-loading="monthlyChartLoading" element-loading-text="" element-loading-spinner="el-icon-loading" element-loading-background="rgba(255, 255, 255, 0.3)">
          <template #header>
            <div class="card-header">
              <el-icon><TrendCharts /></el-icon>
              <span>最近30天工具使用趋势</span>
              <el-button
                size="small"
                :icon="Refresh"
                @click="refreshMonthlyChart"
                :loading="monthlyChartLoading"
              >
                刷新
              </el-button>
            </div>
          </template>
          <div class="chart-container monthly-chart">
            <div ref="monthlyChart" class="trend-chart"></div>
          </div>
        </el-card>
      </el-col>
      </el-row>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted, nextTick } from 'vue'
import { useAppStore } from '@/stores/app'
import { useSystemStore } from '@/stores/system'
import { useServicesStore } from '@/stores/services'
import { useToolsStore } from '@/stores/tools'
import { useToolExecutionStore } from '@/stores/toolExecution'
import { storeServiceAPI } from '@/api/services'
import { agentsAPI } from '@/api/agents'
import { ElMessage } from 'element-plus'
import { Refresh } from '@element-plus/icons-vue'
import ErrorState from '@/components/common/ErrorState.vue'
import * as echarts from 'echarts'

// Store初始化
const appStore = useAppStore()
const systemStore = useSystemStore()
const servicesStore = useServicesStore()
const toolsStore = useToolsStore()
const toolExecutionStore = useToolExecutionStore()

// 响应式数据 - 使用store中的加载状态
const toolStatsLoading = computed(() => toolExecutionStore.isLoading)
const todayChartLoading = ref(false)
const monthlyChartLoading = ref(false)
const servicesLoading = computed(() => servicesStore.isLoading)

// 图表相关
const todayChart = ref(null)
const monthlyChart = ref(null)
let todayChartInstance = null
let monthlyChartInstance = null

// 错误状态 - 本地错误状态管理
const hasLocalError = ref(false)
const hasError = computed(() =>
  hasLocalError.value || appStore.hasErrors || systemStore.hasErrors || servicesStore.hasErrors || toolsStore.hasErrors
)
const errorType = ref('network')
const errorTitle = ref('')
const errorDescription = ref('')
const errorDetails = ref('')
const showErrorDetails = ref(false)

// 最新错误信息
const latestError = computed(() => {
  const errors = [
    ...appStore.recentErrors,
    ...systemStore.recentErrors,
    ...servicesStore.recentErrors,
    ...toolsStore.recentErrors
  ].sort((a, b) => new Date(b.timestamp) - new Date(a.timestamp))

  return errors[0] || null
})

// 使用store中的计算属性
const systemStatus = computed(() => systemStore.systemStatus)

const serviceStats = computed(() => ({
  total: servicesStore.services.length,
  remote: servicesStore.remoteServices.length,
  local: servicesStore.localServices.length,
  healthy: servicesStore.healthyServices.length,
  unhealthy: servicesStore.failedServices.length
}))

const toolStats = computed(() => {
  const available = toolsStore.availableTools.length
  const todayCalls = toolExecutionStore.todayStats.total

  // 🔍 调试信息
  console.log('🔍 [DEBUG] 工具统计计算:', {
    available,
    todayCalls,
    allTools: toolsStore.tools.length,
    todayStatsDetail: toolExecutionStore.todayStats
  })

  return {
    available,
    todayCalls
  }
})

const agentStats = ref({
  active: 0,
  total: 0
})

// 工具使用统计数据 - 使用store中的数据
const topTools = computed(() => toolExecutionStore.popularTools)
const toolUsageStats = computed(() => ({
  total_executions: toolExecutionStore.statistics.totalExecutions,
  successful_executions: toolExecutionStore.statistics.successfulExecutions,
  failed_executions: toolExecutionStore.statistics.failedExecutions,
  average_response_time: toolExecutionStore.statistics.averageResponseTime
}))

// 健康服务数据
const healthyServices = ref([])

// 简化的系统信息 - 只保留运行时间
const systemInfo = ref({
  uptime: '00:00:00',
  startTime: null
})

// 初始化运行时间
const initializeSystemInfo = () => {
  const storedStartTime = localStorage.getItem('mcpstore_session_start')
  const now = Date.now()

  if (!storedStartTime || (now - parseInt(storedStartTime)) > 24 * 60 * 60 * 1000) {
    localStorage.setItem('mcpstore_session_start', now.toString())
    systemInfo.value.startTime = now
  } else {
    systemInfo.value.startTime = parseInt(storedStartTime)
  }
}

const updateUptime = () => {
  if (systemInfo.value.startTime) {
    const now = Date.now()
    const uptimeMs = now - systemInfo.value.startTime
    const hours = Math.floor(uptimeMs / (1000 * 60 * 60))
    const minutes = Math.floor((uptimeMs % (1000 * 60 * 60)) / (1000 * 60))
    const seconds = Math.floor((uptimeMs % (1000 * 60)) / 1000)
    systemInfo.value.uptime = `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}`
  }
}

// 获取Agent数据
const fetchAgentData = async () => {
  try {
    console.log('🔍 [DEBUG] 开始获取Agent数据...')

    // 使用正确的Agent列表接口
    const response = await agentsAPI.getAgentsList()
    console.log('🔍 [DEBUG] Agent API原始响应:', response)

    // 🔧 修复：正确处理API响应格式
    let agentsSummary = {}
    if (response.data && response.data.success && response.data.data) {
      agentsSummary = response.data.data
      console.log('✅ [DEBUG] 使用 response.data.data')
    } else if (response.data && typeof response.data === 'object') {
      agentsSummary = response.data
      console.log('✅ [DEBUG] 使用 response.data')
    } else {
      console.warn('⚠️ [DEBUG] 无法识别的Agent API响应格式')
      agentsSummary = {}
    }

    console.log('🔍 [DEBUG] 提取的Agent摘要数据:', agentsSummary)

    // 计算Agent统计
    agentStats.value = {
      total: agentsSummary.total_agents || 0,
      active: agentsSummary.active_agents || 0
    }

    console.log('✅ [DEBUG] Agent统计更新:', {
      total: agentStats.value.total,
      active: agentStats.value.active,
      summary: agentsSummary
    })
  } catch (error) {
    console.error('❌ 获取Agent数据失败:', error)
    // 设置默认值
    agentStats.value = {
      total: 0,
      active: 0
    }
  }
}

// 计算Agent统计（用于刷新时）
const calculateAgentStats = () => {
  const agents = systemStore.agents || []
  agentStats.value = {
    total: agents.length,
    active: agents.filter(agent => agent.status === 'active' || agent.status === 'healthy').length
  }
}

// 格式化最后执行时间
const formatLastExecuted = (timestamp) => {
  if (!timestamp) return '未知'

  const date = new Date(timestamp)
  const now = new Date()
  const diffMs = now - date
  const diffMinutes = Math.floor(diffMs / (1000 * 60))
  const diffHours = Math.floor(diffMs / (1000 * 60 * 60))
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24))

  if (diffMinutes < 1) return '刚刚'
  if (diffMinutes < 60) return `${diffMinutes}分钟前`
  if (diffHours < 24) return `${diffHours}小时前`
  if (diffDays < 7) return `${diffDays}天前`

  return date.toLocaleDateString('zh-CN')
}

// 获取成功率样式类
const getSuccessRateClass = (rate) => {
  if (rate >= 95) return 'success-high'
  if (rate >= 80) return 'success-medium'
  return 'success-low'
}

// 获取服务类型
const getServiceType = (service) => {
  if (service.url) return 'HTTP服务'
  if (service.command) return '本地服务'
  return '未知类型'
}

// 获取健康服务列表
const fetchHealthyServices = async () => {
  try {
    const services = systemStore.services || []
    const tools = systemStore.tools || []

    // 统计每个服务的工具数量
    const serviceToolCounts = {}
    tools.forEach(tool => {
      const serviceName = tool.service_name || 'unknown'
      serviceToolCounts[serviceName] = (serviceToolCounts[serviceName] || 0) + 1
    })

    // 过滤健康服务并添加工具数量
    healthyServices.value = services
      .filter(service => service.status === 'healthy')
      .map(service => ({
        ...service,
        toolCount: serviceToolCounts[service.name] || 0
      }))
      .slice(0, 10) // 最多显示10个

    console.log('健康服务列表:', healthyServices.value)
  } catch (error) {
    console.error('获取健康服务失败:', error)
    healthyServices.value = []
  }
}

// 刷新健康服务
const refreshHealthyServices = async () => {
  servicesLoading.value = true
  try {
    await systemStore.fetchSystemStatus()
    await fetchHealthyServices()
    ElMessage.success('健康服务刷新成功')
  } catch (error) {
    ElMessage.error('健康服务刷新失败')
  } finally {
    servicesLoading.value = false
  }
}

// 方法
const refreshData = async () => {
  try {
    await systemStore.refreshAllData()
    calculateAgentStats() // 重新计算Agent统计
    ElMessage.success('数据刷新成功')
  } catch (error) {
    ElMessage.error('数据刷新失败')
  }
}

// 防止重复加载的标志
let isLoadingDashboard = false

const loadDashboardData = async () => {
  // 防止重复加载
  if (isLoadingDashboard) {
    console.log('🔍 [DEBUG] Dashboard正在加载中，跳过重复请求')
    return
  }

  try {
    isLoadingDashboard = true
    appStore.setLoadingState('dashboard', true)
    // 清除本地错误状态
    hasLocalError.value = false

    console.log('🔍 [DEBUG] 开始加载Dashboard数据...')

    // 分步加载，避免并发过多导致问题
    try {
      await systemStore.fetchSystemStatus()
      console.log('✅ 系统状态加载完成')
    } catch (error) {
      console.error('❌ 系统状态加载失败:', error)
    }

    try {
      await Promise.all([
        systemStore.fetchServices(true),
        servicesStore.fetchServices(true)
      ])
      console.log('✅ 服务数据加载完成')
    } catch (error) {
      console.error('❌ 服务数据加载失败:', error)
    }

    try {
      await Promise.all([
        systemStore.fetchTools(true),
        toolsStore.fetchTools(true)
      ])
      console.log('✅ 工具数据加载完成')
    } catch (error) {
      console.error('❌ 工具数据加载失败:', error)
    }

    // 获取工具执行记录（可选，失败不影响主要功能）
    try {
      await toolExecutionStore.fetchToolRecords(50, true)
      console.log('✅ 工具执行记录加载完成')
    } catch (error) {
      console.error('❌ 获取工具执行记录失败:', error)
      // 不阻止其他功能
    }

    // 获取Agent数据（可选）
    try {
      await fetchAgentData()
      console.log('✅ Agent数据加载完成')
    } catch (error) {
      console.error('❌ Agent数据加载失败:', error)
    }

    // 获取健康服务列表（可选）
    try {
      await fetchHealthyServices()
      console.log('✅ 健康服务列表加载完成')
    } catch (error) {
      console.error('❌ 健康服务列表加载失败:', error)
    }

    // 加载图表数据（可选）
    try {
      await loadChartData()
      console.log('✅ 图表数据加载完成')
    } catch (error) {
      console.error('❌ 图表数据加载失败:', error)
    }

    console.log('🎯 Dashboard data loaded successfully')

  } catch (error) {
    console.error('❌ 加载仪表板数据失败:', error)
    handleError(error)
  } finally {
    appStore.setLoadingState('dashboard', false)
    isLoadingDashboard = false // 重置加载标志
  }
}

// 错误处理函数
const handleError = (error) => {
  hasLocalError.value = true

  if (error.code === 'ECONNREFUSED' || error.code === 'ERR_NETWORK') {
    errorType.value = 'network'
    errorTitle.value = '无法连接到后端服务'
    errorDescription.value = '请检查后端服务是否正常运行，或稍后重试'
  } else if (error.response?.status >= 500) {
    errorType.value = 'server'
    errorTitle.value = '服务器内部错误'
    errorDescription.value = '服务器遇到了问题，请稍后重试'
  } else if (error.response?.status === 404) {
    errorType.value = 'server'
    errorTitle.value = '接口不存在'
    errorDescription.value = '请求的接口不存在，请检查后端服务版本'
  } else if (error.code === 'ECONNABORTED' || error.message?.includes('timeout')) {
    errorType.value = 'network'
    errorTitle.value = '请求超时'
    errorDescription.value = '网络连接超时，请检查网络状况或稍后重试'
  } else {
    errorType.value = 'unknown'
    errorTitle.value = '加载失败'
    errorDescription.value = '数据加载失败，请稍后重试'
  }

  // 显示错误详情（开发环境）
  if (import.meta.env.DEV) {
    showErrorDetails.value = true
    errorDetails.value = `错误类型: ${error.name || 'Unknown'}
错误消息: ${error.message || '无详细信息'}
错误代码: ${error.code || 'N/A'}
状态码: ${error.response?.status || 'N/A'}
请求URL: ${error.config?.url || 'N/A'}`
  }
}

// 重试处理
const handleRetry = async () => {
  // 防止频繁重试
  if (isLoadingDashboard) {
    ElMessage.warning('正在加载中，请稍候...')
    return
  }

  try {
    // 清除本地错误状态
    hasLocalError.value = false
    await loadDashboardData()
    ElMessage.success('数据重新加载成功')
  } catch (error) {
    console.error('重试失败:', error)
    ElMessage.error('数据重新加载失败，请检查网络连接')
  }
}

// 刷新工具统计
const refreshToolStats = async () => {
  try {
    toolExecutionStore.setLoading('records', true)

    // 刷新工具执行记录
    await toolExecutionStore.fetchToolRecords(50, true)

    // 刷新工具列表
    await toolsStore.fetchTools(true)

    appStore.addNotification({
      title: '工具统计刷新成功',
      message: `已更新 ${toolExecutionStore.popularTools.length} 个热门工具`,
      type: 'success'
    })
  } catch (error) {
    appStore.addError({
      message: `工具统计刷新失败: ${error.message}`,
      type: 'refresh-error',
      source: 'Dashboard.vue'
    })
    ElMessage.error('工具统计刷新失败')
  } finally {
    toolExecutionStore.setLoading('records', false)
  }
}

// 解析时间戳并生成基于真实数据的趋势
const parseToolExecutionTime = (toolsData) => {
  const parsedData = toolsData.map(tool => ({
    ...tool,
    lastExecutedDate: new Date(tool.last_executed)
  }))

  console.log('解析的工具执行时间:', parsedData.map(t => ({
    tool: t.tool_name,
    time: t.lastExecutedDate.toLocaleString('zh-CN'),
    count: t.execution_count
  })))

  return parsedData
}

// 初始化今日24小时趋势图
const initTodayChart = async () => {
  if (!todayChart.value) return

  todayChartInstance = echarts.init(todayChart.value)

  // 基于真实数据生成今日24小时趋势
  const generateTodayTrendData = async () => {
    const data = []
    const now = new Date()
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate())

    // 获取真实的工具执行记录
    let realToolRecords = { executions: [], summary: { total_executions: 0, by_tool: {}, by_service: {} } }
    try {
      realToolRecords = await systemStore.fetchToolRecords(100)
      console.log('今日趋势 - 原始工具记录:', realToolRecords)
    } catch (error) {
      console.warn('无法获取真实工具记录:', error)
    }

    // 统计今天执行的工具（基于execution_time）
    const todayExecutions = realToolRecords.executions.filter(execution => {
      if (!execution.execution_time) return false
      const executionDate = new Date(execution.execution_time)
      const executionDay = new Date(executionDate.getFullYear(), executionDate.getMonth(), executionDate.getDate())
      return executionDay.getTime() === today.getTime()
    })

    console.log('今天执行的工具记录:', todayExecutions)

    // 生成24小时数据点（每小时一个点）
    for (let hour = 0; hour < 24; hour++) {
      const timeLabel = `${hour.toString().padStart(2, '0')}:00`
      let hourlyCount = 0

      // 统计这个小时的执行次数
      todayExecutions.forEach(execution => {
        const executionDate = new Date(execution.execution_time)
        const executionHour = executionDate.getHours()
        if (executionHour === hour) {
          hourlyCount += 1
        }
      })

      data.push([timeLabel, hourlyCount])
    }

    return data
  }

  const todayTrendData = await generateTodayTrendData()

  const todayOption = {
    title: {
      text: '今日工具调用趋势',
      left: 'center',
      textStyle: {
        fontSize: 14,
        color: '#333'
      }
    },
    tooltip: {
      trigger: 'axis',
      formatter: function(params) {
        const data = params[0]
        return `${data.name}<br/>调用次数: ${data.value[1]}`
      }
    },
    grid: {
      left: '8%',
      right: '4%',
      bottom: '15%',
      top: '15%',
      containLabel: true
    },
    xAxis: {
      type: 'category',
      boundaryGap: false,
      axisLabel: {
        fontSize: 10,
        color: '#666'
      },
      axisLine: {
        lineStyle: {
          color: '#e0e0e0'
        }
      }
    },
    yAxis: {
      type: 'value',
      name: '调用次数',
      nameTextStyle: {
        fontSize: 10,
        color: '#666'
      },
      axisLabel: {
        fontSize: 10,
        color: '#666'
      },
      axisLine: {
        lineStyle: {
          color: '#e0e0e0'
        }
      },
      splitLine: {
        lineStyle: {
          color: '#f0f0f0'
        }
      }
    },
    series: [{
      name: '今日工具调用',
      type: 'line',
      smooth: true,
      symbol: 'circle',
      symbolSize: 4,
      lineStyle: {
        color: '#409EFF',
        width: 2
      },
      itemStyle: {
        color: '#409EFF'
      },
      areaStyle: {
        color: {
          type: 'linear',
          x: 0,
          y: 0,
          x2: 0,
          y2: 1,
          colorStops: [{
            offset: 0, color: 'rgba(64, 158, 255, 0.3)'
          }, {
            offset: 1, color: 'rgba(64, 158, 255, 0.1)'
          }]
        }
      },
      data: todayTrendData
    }]
  }

  todayChartInstance.setOption(todayOption)

  // 监听窗口大小变化
  window.addEventListener('resize', () => {
    todayChartInstance?.resize()
  })
}

// 初始化30天趋势图
const initMonthlyChart = async () => {
  if (!monthlyChart.value) return

  monthlyChartInstance = echarts.init(monthlyChart.value)

  // 基于真实数据生成30天趋势
  const generateMonthlyTrendData = async () => {
    const data = []
    const now = new Date()

    // 获取真实的工具执行记录
    let realToolRecords = { executions: [], summary: { total_executions: 0, by_tool: {}, by_service: {} } }
    try {
      const response = await systemStore.fetchToolRecords(500) // 获取更多记录用于月度统计
      console.log('月度趋势 - 原始工具记录:', response)

      // 确保数据结构正确
      if (response && response.executions && Array.isArray(response.executions)) {
        realToolRecords = response
      } else {
        console.warn('工具记录数据格式不正确，使用默认结构')
      }
    } catch (error) {
      console.warn('无法获取真实工具记录:', error)
    }

    // 生成过去30天的数据点
    for (let i = 29; i >= 0; i--) {
      const date = new Date(now.getTime() - i * 24 * 60 * 60 * 1000)
      const dateStr = date.toLocaleDateString('zh-CN', { month: '2-digit', day: '2-digit' })

      // 统计这一天的工具执行次数
      let dailyCount = 0

      // 安全访问executions数组
      if (realToolRecords.executions && Array.isArray(realToolRecords.executions)) {
        realToolRecords.executions.forEach(execution => {
          if (!execution.execution_time) return
          const executionDate = new Date(execution.execution_time)
          const executionDay = new Date(executionDate.getFullYear(), executionDate.getMonth(), executionDate.getDate())
          const targetDate = new Date(date.getFullYear(), date.getMonth(), date.getDate())

          if (executionDay.getTime() === targetDate.getTime()) {
            dailyCount += 1
          }
        })
      }

      // 不生成模拟数据，没有真实数据就显示0

      data.push([dateStr, dailyCount])
    }

    return data
  }

  const monthlyTrendData = await generateMonthlyTrendData()

  const monthlyOption = {
    title: {
      text: '最近30天工具调用趋势',
      left: 'center',
      textStyle: {
        fontSize: 14,
        color: '#333'
      }
    },
    tooltip: {
      trigger: 'axis',
      formatter: function(params) {
        const data = params[0]
        return `${data.name}<br/>调用次数: ${data.value[1]}`
      }
    },
    grid: {
      left: '5%',
      right: '4%',
      bottom: '15%',
      top: '15%',
      containLabel: true
    },
    xAxis: {
      type: 'category',
      boundaryGap: false,
      axisLabel: {
        fontSize: 10,
        color: '#666',
        rotate: 45
      },
      axisLine: {
        lineStyle: {
          color: '#e0e0e0'
        }
      }
    },
    yAxis: {
      type: 'value',
      name: '调用次数',
      nameTextStyle: {
        fontSize: 10,
        color: '#666'
      },
      axisLabel: {
        fontSize: 10,
        color: '#666'
      },
      axisLine: {
        lineStyle: {
          color: '#e0e0e0'
        }
      },
      splitLine: {
        lineStyle: {
          color: '#f0f0f0'
        }
      }
    },
    series: [{
      name: '月度工具调用',
      type: 'line',
      smooth: true,
      symbol: 'circle',
      symbolSize: 3,
      lineStyle: {
        color: '#67C23A',
        width: 2
      },
      itemStyle: {
        color: '#67C23A'
      },
      areaStyle: {
        color: {
          type: 'linear',
          x: 0,
          y: 0,
          x2: 0,
          y2: 1,
          colorStops: [{
            offset: 0, color: 'rgba(103, 194, 58, 0.3)'
          }, {
            offset: 1, color: 'rgba(103, 194, 58, 0.1)'
          }]
        }
      },
      data: monthlyTrendData
    }]
  }

  monthlyChartInstance.setOption(monthlyOption)

  // 监听窗口大小变化
  window.addEventListener('resize', () => {
    monthlyChartInstance?.resize()
  })
}

// 加载图表数据
const loadChartData = async () => {
  try {
    console.log('🔍 开始加载图表数据...')

    // 初始化图表
    await nextTick()
    await initTodayChart()
    await initMonthlyChart()

    console.log('✅ 图表数据加载完成')
  } catch (error) {
    console.error('❌ 图表数据加载失败:', error)
    // 不抛出错误，避免影响整个仪表板加载
  }
}

// 刷新今日趋势图
const refreshTodayChart = async () => {
  todayChartLoading.value = true
  try {
    if (todayChartInstance) {
      todayChartInstance.dispose()
    }
    await nextTick()
    initTodayChart()
    ElMessage.success('今日趋势图刷新成功')
  } catch (error) {
    ElMessage.error('今日趋势图刷新失败')
  } finally {
    todayChartLoading.value = false
  }
}

// 刷新月度趋势图
const refreshMonthlyChart = async () => {
  monthlyChartLoading.value = true
  try {
    if (monthlyChartInstance) {
      monthlyChartInstance.dispose()
    }
    await nextTick()
    initMonthlyChart()
    ElMessage.success('月度趋势图刷新成功')
  } catch (error) {
    ElMessage.error('月度趋势图刷新失败')
  } finally {
    monthlyChartLoading.value = false
  }
}

// 定时器
let uptimeTimer = null

// 生命周期
onMounted(async () => {
  // 初始化系统信息
  initializeSystemInfo()

  // 静默加载仪表板数据
  loadDashboardData().catch(error => {
    console.error('仪表板数据加载失败:', error)
  })

  // 启动运行时间定时器
  uptimeTimer = setInterval(updateUptime, 1000)

  // 初始化图表
  await nextTick()
  initTodayChart()
  initMonthlyChart()
})

onUnmounted(() => {
  // 清理定时器
  if (uptimeTimer) {
    clearInterval(uptimeTimer)
  }

  // 清理图表
  if (todayChartInstance) {
    todayChartInstance.dispose()
  }
  if (monthlyChartInstance) {
    monthlyChartInstance.dispose()
  }

  // 移除窗口监听器
  window.removeEventListener('resize', () => {
    todayChartInstance?.resize()
    monthlyChartInstance?.resize()
  })
})
</script>

<style scoped>
.dashboard {
  padding: 16px;
  background-color: #f5f7fa;
  min-height: calc(100vh - 60px);
}

/* 紧凑卡片样式 */
.compact-card {
  height: 130px; /* 稍微增加高度以适应按钮 */
}

.compact-card .card-header {
  padding: 8px 12px;
  font-size: 13px;
}

.compact-card .card-content {
  padding: 8px 12px;
}

.compact-card .status-item {
  margin-bottom: 4px;
}

.compact-card .status-item .label {
  font-size: 12px;
}

.compact-card .status-item .value {
  font-size: 14px;
}

/* 快捷操作 - 2行2列网格 */
.quick-actions-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  grid-template-rows: 1fr 1fr;
  gap: 8px;
  padding: 8px;
  height: 80px;
  align-items: stretch;
  justify-items: stretch;
  box-sizing: border-box;
  /* 调试边框 - 可以临时启用查看网格 */
  /* border: 1px solid red; */
}

/* 调试网格项 - 可以临时启用 */
/* .quick-actions-grid > * {
  border: 1px solid blue;
} */

.quick-actions-grid .el-button {
  /* 强制重置所有可能影响对齐的属性 */
  width: 100% !important;
  height: 100% !important;
  font-size: 12px !important;
  padding: 0 !important;
  margin: 0 !important;
  border-radius: 4px !important;
  font-weight: 500 !important;
  box-sizing: border-box !important;
  display: flex !important;
  align-items: center !important;
  justify-content: center !important;
  white-space: nowrap !important;
  vertical-align: baseline !important;
  line-height: 1 !important;
  min-height: unset !important;
  max-height: unset !important;
}

/* 特定按钮定位 */
.quick-actions-grid .el-button:nth-child(1) {
  grid-column: 1;
  grid-row: 1;
}

.quick-actions-grid .el-button:nth-child(2) {
  grid-column: 2;
  grid-row: 1;
}

.quick-actions-grid .el-button:nth-child(3) {
  grid-column: 1;
  grid-row: 2;
}

.quick-actions-grid .el-button:nth-child(4) {
  grid-column: 2;
  grid-row: 2;
}

.quick-actions-grid .el-button .el-icon {
  margin-right: 4px !important;
  font-size: 12px !important;
  flex-shrink: 0 !important;
}

.quick-actions-grid .el-button span {
  font-size: 12px !important;
  line-height: 1 !important;
  white-space: nowrap !important;
}

/* 服务统计网格 - 8列宽度 */
.service-stats-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  grid-template-rows: 1fr 1fr;
  gap: 8px;
  padding: 8px 0;
  height: 70px;
}

.stat-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
}

.stat-label {
  font-size: 11px;
  color: #909399;
  margin-bottom: 2px;
}

.stat-value {
  font-size: 16px;
  font-weight: 600;
  color: #303133;
}

.stat-value.text-primary {
  color: #409eff;
}

.stat-value.text-info {
  color: #909399;
}

.stat-value.text-success {
  color: #67c23a;
}

.status-card {
  height: 160px;
}

.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  font-weight: 600;
}

.card-header .el-button {
  margin-left: auto;
}

.card-content {
  margin-top: 16px;
}

.status-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}

.status-item .label {
  color: var(--el-text-color-regular);
  font-size: 14px;
}

.status-item .value {
  font-weight: 600;
  font-size: 16px;
}

.text-success {
  color: var(--el-color-success);
}

.text-danger {
  color: var(--el-color-danger);
}

.quick-actions {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
}

.system-info .info-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 0;
  border-bottom: 1px solid var(--el-border-color-lighter);
}

.system-info .info-item:last-child {
  border-bottom: none;
}

.system-info .label {
  color: var(--el-text-color-regular);
}

.system-info .value {
  font-weight: 600;
}

/* 工具统计样式 */
.tool-stats-list {
  max-height: 300px;
  overflow-y: auto;
}

.tool-stat-item {
  display: flex;
  align-items: center;
  padding: 12px 0;
  border-bottom: 1px solid var(--el-border-color-lighter);
  gap: 12px;
}

.tool-stat-item:last-child {
  border-bottom: none;
}

.tool-rank {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: var(--el-color-primary-light-8);
  color: var(--el-color-primary);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 600;
  flex-shrink: 0;
}

.tool-info {
  flex: 1;
  min-width: 0;
}

.tool-name {
  font-weight: 500;
  color: var(--el-text-color-primary);
  font-size: 14px;
  margin-bottom: 2px;
}

.tool-service {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.tool-metrics {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 2px;
  flex-shrink: 0;
}

.execution-count {
  font-weight: 600;
  color: var(--el-color-primary);
  font-size: 14px;
}

.success-rate {
  font-size: 12px;
  color: var(--el-color-success);
}

.avg-time {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.empty-stats {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px 20px;
  color: var(--el-text-color-secondary);
  gap: 8px;
}

.empty-stats .el-icon {
  font-size: 32px;
  opacity: 0.5;
}

/* 图表容器样式 */
.chart-container {
  height: 300px;
  padding: 10px;
}

.chart-container.today-chart {
  height: 300px; /* 与其他第二行模块保持一致 */
}

.chart-container.monthly-chart {
  height: 350px; /* 月度图表稍高一些 */
}

.trend-chart {
  width: 100%;
  height: 100%;
}

/* 第二行统一卡片高度 */
.logs-card,
.services-card,
.chart-card {
  height: 380px; /* 统一高度 */
}

/* 工具日志样式 */
.tool-logs-container {
  height: 300px; /* 固定容器高度 */
  display: flex;
  flex-direction: column;
}

.tool-logs-list {
  flex: 1;
  overflow-y: auto;
  min-height: 0; /* 允许flex子项收缩 */
}

.tool-log-item {
  padding: 12px;
  border-bottom: 1px solid #f0f0f0;
  transition: background-color 0.2s;
}

.tool-log-item:hover {
  background-color: #f8f9fa;
}

.tool-log-item:last-child {
  border-bottom: none;
}

.tool-log-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}

.tool-name {
  font-weight: 600;
  color: #303133;
  font-size: 14px;
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tool-time {
  font-size: 12px;
  color: #909399;
}

.tool-log-details {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.service-tag {
  background-color: #e1f3d8;
  color: #67c23a;
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 11px;
  font-weight: 500;
}

.execution-count {
  background-color: #ecf5ff;
  color: #409eff;
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 11px;
  font-weight: 500;
}

.success-rate {
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 11px;
  font-weight: 500;
}

.success-rate.success-high {
  background-color: #f0f9ff;
  color: #67c23a;
}

.success-rate.success-medium {
  background-color: #fdf6ec;
  color: #e6a23c;
}

.success-rate.success-low {
  background-color: #fef0f0;
  color: #f56c6c;
}

.response-time {
  background-color: #f4f4f5;
  color: #909399;
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 11px;
  font-weight: 500;
}

.empty-logs {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%; /* 填满整个容器 */
  color: var(--el-text-color-secondary);
  gap: 8px;
}

.empty-logs .el-icon {
  font-size: 32px;
  opacity: 0.5;
}

/* 健康服务样式 */
.healthy-services-container {
  height: 300px; /* 固定容器高度 */
  display: flex;
  flex-direction: column;
}

.healthy-services-list {
  flex: 1;
  overflow-y: auto;
  min-height: 0; /* 允许flex子项收缩 */
}

.service-item {
  display: flex;
  align-items: center;
  padding: 12px;
  border-bottom: 1px solid #f0f0f0;
  transition: background-color 0.2s;
}

.service-item:hover {
  background-color: #f8f9fa;
}

.service-item:last-child {
  border-bottom: none;
}

.service-status {
  margin-right: 12px;
}

.status-icon.healthy {
  color: #67c23a;
  font-size: 16px;
}

.service-info {
  flex: 1;
}

.service-name {
  font-weight: 600;
  color: #303133;
  font-size: 14px;
  margin-bottom: 4px;
}

.service-type {
  font-size: 12px;
  color: #909399;
}

.service-tools {
  margin-left: 8px;
}

.empty-services {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%; /* 填满整个容器 */
  color: var(--el-text-color-secondary);
  gap: 8px;
}

.empty-services .el-icon {
  font-size: 32px;
  opacity: 0.5;
}
</style>
