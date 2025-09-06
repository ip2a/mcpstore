<template>
  <div class="dashboard animate-fade-in">
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
    <div v-else class="dashboard-content">
      <!-- 第一行：统计卡片 -->
      <div class="stats-grid">
        <!-- 系统状态卡片 -->
        <div class="stat-card system-status-card hover-lift">
          <div class="stat-card-header">
            <div class="stat-icon system-icon">
              <el-icon><Monitor /></el-icon>
            </div>
            <div class="stat-info">
              <h3 class="stat-title">系统状态</h3>
              <p class="stat-subtitle">System Status</p>
            </div>
          </div>
          <div class="stat-card-content">
            <div class="status-indicator">
              <span class="status-label">运行状态</span>
              <el-tag 
                :type="systemStatus.running ? 'success' : 'danger'" 
                size="small"
                :effect="systemStatus.running ? 'light' : 'plain'"
                class="status-tag"
              >
                <el-icon class="status-tag-icon">
                  <component :is="systemStatus.running ? 'CircleCheck' : 'CircleClose'" />
                </el-icon>
                {{ systemStatus.running ? '运行中' : '已停止' }}
              </el-tag>
            </div>
            <div class="status-metric">
              <span class="metric-label">运行时间</span>
              <span class="metric-value">{{ systemInfo.uptime }}</span>
            </div>
          </div>
        </div>

        <!-- 快速操作卡片 -->
        <div class="stat-card quick-actions-card hover-lift">
          <div class="stat-card-header">
            <div class="stat-icon actions-icon">
              <el-icon><Operation /></el-icon>
            </div>
            <div class="stat-info">
              <h3 class="stat-title">快速操作</h3>
              <p class="stat-subtitle">Quick Actions</p>
            </div>
          </div>
          <div class="quick-actions-grid">
            <el-button 
              size="small" 
              type="primary" 
              @click="$router.push('/services/add')"
              class="action-btn primary-action"
            >
              <el-icon><Plus /></el-icon>
              添加服务
            </el-button>
            <el-button 
              size="small" 
              type="success" 
              @click="$router.push('/tools/execute')"
              class="action-btn success-action"
            >
              <el-icon><VideoPlay /></el-icon>
              执行工具
            </el-button>
            <el-button 
              size="small" 
              type="info" 
              @click="$router.push('/agents/create')"
              class="action-btn info-action"
            >
              <el-icon><UserFilled /></el-icon>
              创建Agent
            </el-button>
            <el-button 
              size="small" 
              type="warning" 
              @click="refreshData"
              class="action-btn warning-action"
            >
              <el-icon><Refresh /></el-icon>
              刷新数据
            </el-button>
          </div>
        </div>

        <!-- 工具统计卡片 -->
        <div class="stat-card tools-stats-card hover-lift">
          <div class="stat-card-header">
            <div class="stat-icon tools-icon">
              <el-icon><Tools /></el-icon>
            </div>
            <div class="stat-info">
              <h3 class="stat-title">工具统计</h3>
              <p class="stat-subtitle">Tools Statistics</p>
            </div>
          </div>
          <div class="stat-card-content">
            <div class="metric-row">
              <span class="metric-label">可用工具</span>
              <span class="metric-value highlight">{{ toolStats.available }}</span>
            </div>
            <div class="metric-row">
              <span class="metric-label">今日调用</span>
              <span class="metric-value success">{{ toolStats.todayCalls }}</span>
            </div>
          </div>
        </div>

        <!-- Agent统计卡片 -->
        <div class="stat-card agents-stats-card hover-lift">
          <div class="stat-card-header">
            <div class="stat-icon agents-icon">
              <el-icon><User /></el-icon>
            </div>
            <div class="stat-info">
              <h3 class="stat-title">Agent统计</h3>
              <p class="stat-subtitle">Agents Statistics</p>
            </div>
          </div>
          <div class="stat-card-content">
            <div class="metric-row">
              <span class="metric-label">活跃Agent</span>
              <span class="metric-value success">{{ agentStats.active }}</span>
            </div>
            <div class="metric-row">
              <span class="metric-label">总Agent数</span>
              <span class="metric-value">{{ agentStats.total }}</span>
            </div>
          </div>
        </div>

        <!-- 服务统计卡片 -->
        <div class="stat-card services-stats-card hover-lift">
          <div class="stat-card-header">
            <div class="stat-icon services-icon">
              <el-icon><Connection /></el-icon>
            </div>
            <div class="stat-info">
              <h3 class="stat-title">服务统计</h3>
              <p class="stat-subtitle">Services Statistics</p>
            </div>
          </div>
          <div class="services-stats-grid">
            <div class="service-stat-item">
              <div class="service-stat-value">{{ serviceStats.total }}</div>
              <div class="service-stat-label">总服务数</div>
            </div>
            <div class="service-stat-item">
              <div class="service-stat-value primary">{{ serviceStats.remote }}</div>
              <div class="service-stat-label">远程服务</div>
            </div>
            <div class="service-stat-item">
              <div class="service-stat-value info">{{ serviceStats.local }}</div>
              <div class="service-stat-label">本地服务</div>
            </div>
            <div class="service-stat-item">
              <div class="service-stat-value success">{{ serviceStats.healthy }}</div>
              <div class="service-stat-label">健康服务</div>
            </div>
          </div>
        </div>
      </div>

      <!-- 第二行：详细数据展示 -->
      <div class="data-section">
        <!-- 工具使用日志 -->
        <div class="data-card logs-card hover-lift">
          <div class="data-card-header">
            <div class="data-card-title">
              <el-icon class="data-card-icon"><Tools /></el-icon>
              <div class="title-content">
                <h4 class="data-card-heading">工具使用日志</h4>
                <p class="data-card-subtitle">Recent Tool Usage</p>
              </div>
            </div>
            <el-button
              size="small"
              :icon="Refresh"
              @click="refreshToolStats"
              :loading="toolStatsLoading"
              class="refresh-btn"
            >
              刷新
            </el-button>
          </div>
          <div class="data-card-content">
            <div v-if="toolStatsLoading" class="loading-container">
              <el-icon class="loading-icon"><Loading /></el-icon>
              <span class="loading-text">加载工具使用数据...</span>
            </div>
            <div v-else class="tool-logs-container">
              <div v-if="topTools.length > 0" class="tool-logs-list">
                <div
                  v-for="tool in topTools"
                  :key="tool.tool_name"
                  class="tool-log-item hover-scale"
                >
                  <div class="tool-log-header">
                    <div class="tool-name-wrapper">
                      <el-icon class="tool-icon"><Setting /></el-icon>
                      <div class="tool-name">{{ tool.tool_name }}</div>
                    </div>
                    <div class="tool-time">{{ formatLastExecuted(tool.last_executed) }}</div>
                  </div>
                  <div class="tool-log-details">
                    <el-tag size="small" type="primary" class="service-tag">
                      {{ tool.service_name }}
                    </el-tag>
                    <div class="metric-badge">
                      <span class="badge-label">调用</span>
                      <span class="badge-value">{{ tool.execution_count }}</span>
                    </div>
                    <div 
                      class="success-rate-badge"
                      :class="getSuccessRateClass(tool.success_rate)"
                    >
                      <el-icon><TrendCharts /></el-icon>
                      {{ tool.success_rate.toFixed(1) }}%
                    </div>
                    <div class="response-time-badge">
                      <el-icon><Timer /></el-icon>
                      {{ tool.average_response_time.toFixed(0) }}ms
                    </div>
                  </div>
                </div>
              </div>
              <div v-else class="empty-container">
                <el-icon class="empty-icon"><Tools /></el-icon>
                <div class="empty-title">暂无工具使用记录</div>
                <div class="empty-description">开始使用工具后，这里将显示使用统计信息</div>
              </div>
            </div>
          </div>
        </div>

        <!-- 健康服务状态 -->
        <div class="data-card health-card hover-lift">
          <div class="data-card-header">
            <div class="data-card-title">
              <el-icon class="data-card-icon health-icon"><CircleCheck /></el-icon>
              <div class="title-content">
                <h4 class="data-card-heading">健康服务</h4>
                <p class="data-card-subtitle">Healthy Services</p>
              </div>
            </div>
            <el-button
              size="small"
              :icon="Refresh"
              @click="refreshHealthyServices"
              :loading="servicesLoading"
              class="refresh-btn"
            >
              刷新
            </el-button>
          </div>
          <div class="data-card-content">
            <div v-if="servicesLoading" class="loading-container">
              <el-icon class="loading-icon"><Loading /></el-icon>
              <span class="loading-text">检查服务状态...</span>
            </div>
            <div v-else class="healthy-services-container">
              <div v-if="healthyServices.length > 0" class="healthy-services-list">
                <div
                  v-for="service in healthyServices"
                  :key="service.name"
                  class="service-item hover-scale"
                >
                  <div class="service-status-indicator">
                    <el-icon class="status-icon healthy-pulse"><CircleCheck /></el-icon>
                  </div>
                  <div class="service-info">
                    <div class="service-name">{{ service.name }}</div>
                    <div class="service-type">{{ getServiceType(service) }}</div>
                  </div>
                  <div class="service-tools">
                    <el-tag size="small" type="success" effect="light">
                      {{ service.toolCount || 0 }} 工具
                    </el-tag>
                  </div>
                </div>
              </div>
              <div v-else class="empty-container">
                <el-icon class="empty-icon"><Warning /></el-icon>
                <div class="empty-title">暂无健康服务</div>
                <div class="empty-description">所有服务当前都处于异常状态</div>
              </div>
            </div>
          </div>
        </div>

        <!-- 今日24小时趋势图 -->
        <div class="data-card chart-card hover-lift">
          <div class="data-card-header">
            <div class="data-card-title">
              <el-icon class="data-card-icon chart-icon"><TrendCharts /></el-icon>
              <div class="title-content">
                <h4 class="data-card-heading">今日趋势</h4>
                <p class="data-card-subtitle">24-Hour Usage Trend</p>
              </div>
            </div>
            <el-button
              size="small"
              :icon="Refresh"
              @click="refreshTodayChart"
              :loading="todayChartLoading"
              class="refresh-btn"
            >
              刷新
            </el-button>
          </div>
          <div class="data-card-content">
            <div v-if="todayChartLoading" class="loading-container">
              <el-icon class="loading-icon"><Loading /></el-icon>
              <span class="loading-text">加载趋势数据...</span>
            </div>
            <div v-else class="chart-container today-chart">
              <div ref="todayChart" class="trend-chart"></div>
            </div>
          </div>
        </div>
      </div>

      <!-- 第三行：30天趋势图 -->
      <div class="data-section full-width">
        <div class="data-card monthly-chart-card hover-lift">
          <div class="data-card-header">
            <div class="data-card-title">
              <el-icon class="data-card-icon chart-icon"><TrendCharts /></el-icon>
              <div class="title-content">
                <h4 class="data-card-heading">最近30天工具使用趋势</h4>
                <p class="data-card-subtitle">30-Day Usage Trend</p>
              </div>
            </div>
            <el-button
              size="small"
              :icon="Refresh"
              @click="refreshMonthlyChart"
              :loading="monthlyChartLoading"
              class="refresh-btn"
            >
              刷新
            </el-button>
          </div>
          <div class="data-card-content">
            <div v-if="monthlyChartLoading" class="loading-container">
              <el-icon class="loading-icon"><Loading /></el-icon>
              <span class="loading-text">加载月度趋势数据...</span>
            </div>
            <div v-else class="chart-container monthly-chart">
              <div ref="monthlyChart" class="trend-chart"></div>
            </div>
          </div>
        </div>
      </div>
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
import { api } from '@/api'
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
    const response = await api.agent.getAgentsList()
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
  try {
    console.log('🔍 [Dashboard] 开始初始化今日图表...')
    console.log('🔍 [Dashboard] 今日图表DOM元素:', todayChart.value)
    
    if (!todayChart.value) {
      console.warn('⚠️ [Dashboard] 今日图表容器不存在')
      return
    }

    // 确保容器有正确的尺寸
    if (todayChart.value.offsetWidth === 0 || todayChart.value.offsetHeight === 0) {
      console.warn('⚠️ [Dashboard] 今日图表容器尺寸为0，等待DOM渲染完成')
      await nextTick()
      await new Promise(resolve => setTimeout(resolve, 100))
    }

    todayChartInstance = echarts.init(todayChart.value)
    console.log('✅ [Dashboard] 今日图表ECharts实例创建成功')

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
  console.log('✅ [Dashboard] 今日图表配置设置成功')

  // 监听窗口大小变化
  window.addEventListener('resize', () => {
    todayChartInstance?.resize()
  })
  
  } catch (error) {
    console.error('❌ [Dashboard] 今日图表初始化失败:', error)
    console.error('❌ [Dashboard] 错误详情:', error.stack)
  }
}

// 初始化30天趋势图
const initMonthlyChart = async () => {
  try {
    console.log('🔍 [Dashboard] 开始初始化月度图表...')
    console.log('🔍 [Dashboard] 月度图表DOM元素:', monthlyChart.value)
    
    if (!monthlyChart.value) {
      console.warn('⚠️ [Dashboard] 月度图表容器不存在')
      return
    }

    // 确保容器有正确的尺寸
    if (monthlyChart.value.offsetWidth === 0 || monthlyChart.value.offsetHeight === 0) {
      console.warn('⚠️ [Dashboard] 月度图表容器尺寸为0，等待DOM渲染完成')
      await nextTick()
      await new Promise(resolve => setTimeout(resolve, 100))
    }

    monthlyChartInstance = echarts.init(monthlyChart.value)
    console.log('✅ [Dashboard] 月度图表ECharts实例创建成功')

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
  console.log('✅ [Dashboard] 月度图表配置设置成功')

  // 监听窗口大小变化
  window.addEventListener('resize', () => {
    monthlyChartInstance?.resize()
  })
  
  } catch (error) {
    console.error('❌ [Dashboard] 月度图表初始化失败:', error)
    console.error('❌ [Dashboard] 错误详情:', error.stack)
  }
}

// 加载图表数据
const loadChartData = async () => {
  try {
    console.log('🔍 [Dashboard] 开始加载图表数据...')

    // 确保DOM完全渲染
    await nextTick()
    await new Promise(resolve => setTimeout(resolve, 200))

    // 初始化图表
    await initTodayChart()
    await initMonthlyChart()

    console.log('✅ [Dashboard] 图表数据加载完成')
  } catch (error) {
    console.error('❌ [Dashboard] 图表数据加载失败:', error)
    console.error('❌ [Dashboard] 错误详情:', error.stack)
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

<style lang="scss" scoped>
// 仪表板样式 - 现代化专业设计
.dashboard {
  padding: 24px;
  background-color: var(--bg-color-page);
  min-height: calc(100vh - 64px);
  animation: fadeIn 0.6s ease-out;
}

// 统计卡片网格布局
.stats-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  gap: 24px;
  margin-bottom: 32px;
  
  @media (min-width: 1200px) {
    grid-template-columns: repeat(4, 1fr);
  }
  
  @media (max-width: 768px) {
    grid-template-columns: 1fr;
    gap: 16px;
  }
}

// 数据区域网格布局
.data-section {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(400px, 1fr));
  gap: 24px;
  margin-bottom: 32px;
  
  @media (min-width: 1400px) {
    grid-template-columns: repeat(3, 1fr);
  }
  
  @media (max-width: 1200px) {
    grid-template-columns: repeat(2, 1fr);
  }
  
  @media (max-width: 768px) {
    grid-template-columns: 1fr;
    gap: 16px;
  }
}

// 全宽区域
.full-width {
  grid-column: 1 / -1;
}

// 统计卡片基础样式
.stat-card {
  @include card-base;
  padding: 24px;
  background: linear-gradient(135deg, var(--bg-color) 0%, var(--bg-color-secondary) 100%);
  border: 1px solid var(--border-lighter);
  position: relative;
  overflow: hidden;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  
  &::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 4px;
    background: linear-gradient(90deg, var(--primary-color), var(--primary-light));
    transform: scaleX(0);
    transition: transform 0.3s ease;
  }
  
  &:hover {
    transform: translateY(-4px);
    box-shadow: var(--shadow-lg);
    border-color: var(--primary-light);
    
    &::before {
      transform: scaleX(1);
    }
  }
  
  // 特殊卡片样式
  &.system-status-card {
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    color: var(--text-inverse);
    
    .stat-icon {
      background: rgba(255, 255, 255, 0.2);
      color: var(--text-inverse);
    }
    
    .stat-title,
    .stat-subtitle {
      color: var(--text-inverse);
    }
    
    .status-tag {
      background: rgba(255, 255, 255, 0.2);
      border-color: rgba(255, 255, 255, 0.3);
      color: var(--text-inverse);
    }
  }
  
  &.quick-actions-card {
    background: linear-gradient(135deg, #f093fb 0%, #f5576c 100%);
    color: var(--text-inverse);
    
    .stat-icon {
      background: rgba(255, 255, 255, 0.2);
      color: var(--text-inverse);
    }
    
    .stat-title,
    .stat-subtitle {
      color: var(--text-inverse);
    }
  }
  
  &.tools-stats-card {
    background: linear-gradient(135deg, #4facfe 0%, #00f2fe 100%);
    color: var(--text-inverse);
    
    .stat-icon {
      background: rgba(255, 255, 255, 0.2);
      color: var(--text-inverse);
    }
    
    .stat-title,
    .stat-subtitle {
      color: var(--text-inverse);
    }
  }
  
  &.agents-stats-card {
    background: linear-gradient(135deg, #43e97b 0%, #38f9d7 100%);
    color: var(--text-inverse);
    
    .stat-icon {
      background: rgba(255, 255, 255, 0.2);
      color: var(--text-inverse);
    }
    
    .stat-title,
    .stat-subtitle {
      color: var(--text-inverse);
    }
  }
  
  &.services-stats-card {
    background: linear-gradient(135deg, #fa709a 0%, #fee140 100%);
    color: var(--text-inverse);
    
    .stat-icon {
      background: rgba(255, 255, 255, 0.2);
      color: var(--text-inverse);
    }
    
    .stat-title,
    .stat-subtitle {
      color: var(--text-inverse);
    }
  }
}

// 卡片头部
.stat-card-header {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-bottom: 20px;
}

// 图标样式
.stat-icon {
  width: 48px;
  height: 48px;
  border-radius: var(--border-radius-lg);
  background: var(--primary-lighter);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 24px;
  color: var(--primary-color);
  transition: all 0.3s ease;
}

// 卡片信息
.stat-info {
  flex: 1;
}

.stat-title {
  font-size: var(--font-size-lg);
  font-weight: var(--font-weight-semibold);
  color: var(--text-primary);
  margin: 0 0 4px 0;
  line-height: 1.2;
}

.stat-subtitle {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
  margin: 0;
  opacity: 0.8;
}

// 卡片内容
.stat-card-content {
  .status-indicator,
  .metric-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
    
    &:last-child {
      margin-bottom: 0;
    }
  }
  
  .status-label,
  .metric-label {
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
  }
  
  .metric-value {
    font-size: var(--font-size-xl);
    font-weight: var(--font-weight-bold);
    color: var(--text-primary);
    
    &.highlight {
      color: var(--primary-color);
      font-size: var(--font-size-2xl);
    }
    
    &.success {
      color: var(--success-color);
    }
    
    &.primary {
      color: var(--primary-color);
    }
    
    &.info {
      color: var(--info-color);
    }
  }
}

// 快捷操作网格
.quick-actions-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 12px;
  height: 120px;
}

.action-btn {
  width: 100% !important;
  height: 100% !important;
  border-radius: var(--border-radius-md) !important;
  border: 1px solid rgba(255, 255, 255, 0.3) !important;
  background: rgba(255, 255, 255, 0.1) !important;
  color: var(--text-inverse) !important;
  font-size: var(--font-size-sm) !important;
  font-weight: var(--font-weight-medium) !important;
  transition: all 0.3s ease !important;
  display: flex !important;
  flex-direction: column;
  align-items: center !important;
  justify-content: center !important;
  gap: 8px !important;
  
  &:hover {
    background: rgba(255, 255, 255, 0.2) !important;
    transform: translateY(-2px) !important;
    box-shadow: 0 8px 25px rgba(0, 0, 0, 0.15) !important;
  }
  
  .el-icon {
    font-size: 20px !important;
    margin: 0 !important;
  }
  
  span {
    font-size: var(--font-size-sm) !important;
    line-height: 1 !important;
  }
}

// 服务统计网格
.services-stats-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  grid-template-rows: repeat(2, 1fr);
  gap: 12px;
  height: 120px;
}

.service-stat-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  background: rgba(255, 255, 255, 0.1);
  border-radius: var(--border-radius-md);
  border: 1px solid rgba(255, 255, 255, 0.2);
  transition: all 0.3s ease;
  
  &:hover {
    background: rgba(255, 255, 255, 0.15);
    transform: scale(1.05);
  }
  
  .service-stat-value {
    font-size: var(--font-size-xl);
    font-weight: var(--font-weight-bold);
    color: var(--text-inverse);
    margin-bottom: 4px;
    
    &.primary {
      color: #FFE066;
    }
    
    &.info {
      color: #A8DADC;
    }
    
    &.success {
      color: #95E1D3;
    }
  }
  
  .service-stat-label {
    font-size: var(--font-size-xs);
    color: var(--text-inverse);
    opacity: 0.9;
  }
}

// 数据卡片
.data-card {
  @include card-base;
  background: var(--bg-color);
  border: 1px solid var(--border-lighter);
  transition: all 0.3s ease;
  
  &:hover {
    box-shadow: var(--shadow-md);
    border-color: var(--primary-light);
  }
}

// 数据卡片头部
.data-card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 24px;
  border-bottom: 1px solid var(--border-lighter);
  background: linear-gradient(135deg, var(--bg-color) 0%, var(--bg-color-secondary) 100%);
}

.data-card-title {
  display: flex;
  align-items: center;
  gap: 12px;
}

.data-card-icon {
  width: 40px;
  height: 40px;
  border-radius: var(--border-radius-md);
  background: var(--primary-lighter);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 20px;
  color: var(--primary-color);
}

.title-content {
  h4 {
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
    color: var(--text-primary);
    margin: 0 0 2px 0;
  }
  
  p {
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
    margin: 0;
  }
}

.refresh-btn {
  padding: 8px 16px !important;
  border-radius: var(--border-radius-sm) !important;
  font-size: var(--font-size-sm) !important;
}

// 数据卡片内容
.data-card-content {
  padding: 24px;
  min-height: 300px;
}

// 加载状态
.loading-container {
  @include flex-column-center;
  min-height: 300px;
  color: var(--text-secondary);
  
  .loading-icon {
    font-size: 32px;
    margin-bottom: 16px;
    animation: pulse 2s infinite;
  }
  
  .loading-text {
    font-size: var(--font-size-base);
  }
}

// 工具日志
.tool-logs-container {
  height: 300px;
  display: flex;
  flex-direction: column;
}

.tool-logs-list {
  flex: 1;
  overflow-y: auto;
  @include scrollbar-thin;
}

.tool-log-item {
  padding: 16px;
  border-bottom: 1px solid var(--border-lighter);
  transition: all 0.2s ease;
  
  &:hover {
    background: var(--bg-color-secondary);
  }
  
  &:last-child {
    border-bottom: none;
  }
}

.tool-log-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.tool-name-wrapper {
  display: flex;
  align-items: center;
  gap: 8px;
}

.tool-name {
  font-size: var(--font-size-base);
  font-weight: var(--font-weight-semibold);
  color: var(--text-primary);
  @include text-ellipsis;
  max-width: 200px;
}

.tool-time {
  font-size: var(--font-size-xs);
  color: var(--text-secondary);
}

.tool-log-details {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.service-tag {
  background: var(--success-lighter);
  color: var(--success-color);
  padding: 4px 8px;
  border-radius: var(--border-radius-sm);
  font-size: var(--font-size-xs);
  font-weight: var(--font-weight-medium);
}

.metric-badge,
.success-rate-badge,
.response-time-badge {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  border-radius: var(--border-radius-sm);
  font-size: var(--font-size-xs);
  font-weight: var(--font-weight-medium);
}

.metric-badge {
  background: var(--primary-lighter);
  color: var(--primary-color);
}

.success-rate-badge {
  background: var(--success-lighter);
  color: var(--success-color);
  
  &.success-high {
    background: var(--success-lighter);
    color: var(--success-color);
  }
  
  &.success-medium {
    background: var(--warning-lighter);
    color: var(--warning-color);
  }
  
  &.success-low {
    background: var(--danger-lighter);
    color: var(--danger-color);
  }
}

.response-time-badge {
  background: var(--info-lighter);
  color: var(--info-color);
}

// 健康服务
.healthy-services-container {
  height: 300px;
  display: flex;
  flex-direction: column;
}

.healthy-services-list {
  flex: 1;
  overflow-y: auto;
  @include scrollbar-thin;
}

.service-item {
  display: flex;
  align-items: center;
  padding: 16px;
  border-bottom: 1px solid var(--border-lighter);
  transition: all 0.2s ease;
  
  &:hover {
    background: var(--bg-color-secondary);
  }
  
  &:last-child {
    border-bottom: none;
  }
}

.service-status-indicator {
  margin-right: 12px;
  
  .status-icon {
    color: var(--success-color);
    font-size: 20px;
    
    &.healthy-pulse {
      animation: pulse 2s infinite;
    }
  }
}

.service-info {
  flex: 1;
  
  .service-name {
    font-size: var(--font-size-base);
    font-weight: var(--font-weight-semibold);
    color: var(--text-primary);
    margin-bottom: 4px;
  }
  
  .service-type {
    font-size: var(--font-size-xs);
    color: var(--text-secondary);
  }
}

.service-tools {
  margin-left: auto;
}

// 图表容器
.chart-container {
  height: 300px;
  padding: 16px;
  position: relative;
  
  &.monthly-chart {
    height: 350px;
  }
}

.trend-chart {
  width: 100%;
  height: 100%;
}

// 空状态
.empty-container {
  @include flex-column-center;
  height: 100%;
  color: var(--text-secondary);
  text-align: center;
  
  .empty-icon {
    font-size: 48px;
    margin-bottom: 16px;
    opacity: 0.5;
  }
  
  .empty-title {
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
    color: var(--text-primary);
    margin-bottom: 8px;
  }
  
  .empty-description {
    font-size: var(--font-size-base);
    color: var(--text-secondary);
    max-width: 300px;
    line-height: var(--line-height-relaxed);
  }
}

// 状态标签
.status-tag {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 12px;
  border-radius: var(--border-radius-sm);
  font-size: var(--font-size-xs);
  font-weight: var(--font-weight-medium);
  
  .status-tag-icon {
    font-size: 14px;
  }
  
  &.success {
    background: var(--success-lighter);
    color: var(--success-color);
    border: 1px solid var(--success-light);
  }
  
  &.danger {
    background: var(--danger-lighter);
    color: var(--danger-color);
    border: 1px solid var(--danger-light);
  }
}

// 动画
@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(20px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes pulse {
  0%, 100% {
    opacity: 1;
  }
  50% {
    opacity: 0.5;
  }
}

@keyframes shimmer {
  0% {
    transform: translateX(-100%);
  }
  100% {
    transform: translateX(100%);
  }
}

// 响应式设计
@media (max-width: 768px) {
  .dashboard {
    padding: 16px;
  }
  
  .stats-grid {
    grid-template-columns: 1fr;
    gap: 16px;
  }
  
  .data-section {
    grid-template-columns: 1fr;
    gap: 16px;
  }
  
  .stat-card {
    padding: 20px;
  }
  
  .quick-actions-grid {
    grid-template-columns: 1fr;
    height: auto;
    gap: 8px;
  }
  
  .services-stats-grid {
    grid-template-columns: repeat(2, 1fr);
    gap: 8px;
  }
  
  .data-card-header {
    padding: 16px 20px;
  }
  
  .data-card-content {
    padding: 20px;
  }
  
  .tool-log-item,
  .service-item {
    padding: 12px;
  }
}

// 暗色模式适配
:root.dark {
  .stat-card {
    background: linear-gradient(135deg, var(--bg-color) 0%, var(--bg-color-tertiary) 100%);
    border-color: var(--border-light);
  }
  
  .data-card {
    background: var(--bg-color);
    border-color: var(--border-light);
  }
  
  .data-card-header {
    background: linear-gradient(135deg, var(--bg-color) 0%, var(--bg-color-tertiary) 100%);
    border-color: var(--border-light);
  }
  
  .tool-log-item,
  .service-item {
    border-color: var(--border-light);
    
    &:hover {
      background: var(--bg-color-tertiary);
    }
  }
}
</style>
