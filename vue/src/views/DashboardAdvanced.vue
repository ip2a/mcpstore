<template>
  <div class="dashboard-advanced">
    <!-- Loading State -->
    <div v-if="isLoading" class="loading-container">
      <el-skeleton :rows="10" animated />
    </div>

    <!-- Error State -->
    <ErrorState
      v-else-if="hasError"
      :type="errorType"
      :title="errorTitle"
      :description="errorDescription"
      :show-details="showErrorDetails"
      :error-details="errorDetails"
      @retry="handleRetry"
    />

    <!-- Dashboard Content -->
    <div v-else class="dashboard-content">
      <!-- Hero Section with Key Metrics -->
      <div class="hero-section">
        <div class="hero-content">
          <div class="hero-title">
            <h1 class="greeting">{{ greeting }}</h1>
            <p class="subtitle">系统运行状态概览</p>
          </div>
          <div class="hero-metrics">
            <div class="metric-card">
              <div class="metric-icon success">
                <el-icon><CircleCheck /></el-icon>
              </div>
              <div class="metric-content">
                <div class="metric-value">{{ systemStatus.running ? '正常' : '异常' }}</div>
                <div class="metric-label">系统状态</div>
              </div>
            </div>
            <div class="metric-card">
              <div class="metric-icon primary">
                <el-icon><Connection /></el-icon>
              </div>
              <div class="metric-content">
                <div class="metric-value">{{ serviceStats.healthy }}</div>
                <div class="metric-label">健康服务</div>
              </div>
            </div>
            <div class="metric-card">
              <div class="metric-icon info">
                <el-icon><Tools /></el-icon>
              </div>
              <div class="metric-content">
                <div class="metric-value">{{ toolStats.available }}</div>
                <div class="metric-label">可用工具</div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Quick Actions Bar -->
      <div class="quick-actions-bar">
        <div class="section-title">
          <h2>快捷操作</h2>
        </div>
        <div class="quick-actions">
          <el-button
            v-for="action in quickActions"
            :key="action.key"
            :type="action.type"
            :icon="action.icon"
            size="large"
            @click="action.handler"
            class="action-btn"
          >
            {{ action.label }}
          </el-button>
        </div>
      </div>

      <!-- Main Content Grid -->
      <div class="content-grid">
        <!-- Left Column -->
        <div class="left-column">
          <!-- Service Status Overview -->
          <div class="content-card">
            <div class="card-header">
              <h3>
                <el-icon><Connection /></el-icon>
                服务分布
              </h3>
              <el-button text @click="refreshServices">
                <el-icon><Refresh /></el-icon>
              </el-button>
            </div>
            <div class="card-content">
              <div class="service-distribution">
                <div class="distribution-chart">
                  <div
                    v-for="(stat, key) in serviceDistribution"
                    :key="key"
                    class="distribution-item"
                  >
                    <div class="distribution-info">
                      <span class="label">{{ stat.label }}</span>
                      <span class="value">{{ stat.count }}</span>
                    </div>
                    <div class="distribution-bar">
                      <div
                        class="bar-fill"
                        :style="{
                          width: `${stat.percentage}%`,
                          backgroundColor: stat.color
                        }"
                      ></div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <!-- Recent Activity -->
          <div class="content-card">
            <div class="card-header">
              <h3>
                <el-icon><Clock /></el-icon>
                最近活动
              </h3>
              <el-button text @click="refreshActivity">
                <el-icon><Refresh /></el-icon>
              </el-button>
            </div>
            <div class="card-content">
              <div class="activity-list">
                <div
                  v-for="activity in recentActivities"
                  :key="activity.id"
                  class="activity-item"
                >
                  <div class="activity-icon" :class="activity.type">
                    <el-icon><component :is="activity.icon" /></el-icon>
                  </div>
                  <div class="activity-details">
                    <div class="activity-title">{{ activity.title }}</div>
                    <div class="activity-time">{{ formatTime(activity.timestamp) }}</div>
                  </div>
                </div>
                <div v-if="recentActivities.length === 0" class="empty-state">
                  <el-icon><Box /></el-icon>
                  <span>暂无活动记录</span>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Center Column -->
        <div class="center-column">
          <!-- Tool Usage Chart -->
          <div class="content-card chart-card">
            <div class="card-header">
              <h3>
                <el-icon><TrendCharts /></el-icon>
                工具使用趋势
              </h3>
              <div class="chart-controls">
                <el-radio-group v-model="chartPeriod" size="small">
                  <el-radio-button label="today">今日</el-radio-button>
                  <el-radio-button label="week">本周</el-radio-button>
                  <el-radio-button label="month">本月</el-radio-button>
                </el-radio-group>
              </div>
            </div>
            <div class="card-content">
              <div class="chart-container" ref="usageChart"></div>
            </div>
          </div>

          <!-- Popular Tools -->
          <div class="content-card">
            <div class="card-header">
              <h3>
                <el-icon><Star /></el-icon>
                热门工具
              </h3>
              <el-button text @click="refreshTools">
                <el-icon><Refresh /></el-icon>
              </el-button>
            </div>
            <div class="card-content">
              <div class="popular-tools">
                <div
                  v-for="(tool, index) in popularTools"
                  :key="tool.tool_name"
                  class="tool-item"
                >
                  <div class="tool-rank">{{ index + 1 }}</div>
                  <div class="tool-info">
                    <div class="tool-name">{{ tool.tool_name }}</div>
                    <div class="tool-service">{{ tool.service_name }}</div>
                  </div>
                  <div class="tool-stats">
                    <div class="stat">
                      <span class="stat-value">{{ tool.execution_count }}</span>
                      <span class="stat-label">调用</span>
                    </div>
                    <div class="stat">
                      <span class="stat-value">{{ tool.success_rate.toFixed(0) }}%</span>
                      <span class="stat-label">成功率</span>
                    </div>
                  </div>
                </div>
                <div v-if="popularTools.length === 0" class="empty-state">
                  <el-icon><Box /></el-icon>
                  <span>暂无工具数据</span>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Right Column -->
        <div class="right-column">
          <!-- System Resources -->
          <div class="content-card">
            <div class="card-header">
              <h3>
                <el-icon><Monitor /></el-icon>
                系统资源
              </h3>
              <el-button text @click="refreshResources">
                <el-icon><Refresh /></el-icon>
              </el-button>
            </div>
            <div class="card-content">
              <div class="resource-metrics">
                <div class="resource-item">
                  <div class="resource-info">
                    <span class="resource-label">CPU 使用率</span>
                    <span class="resource-value">{{ systemResources.cpu || 0 }}%</span>
                  </div>
                  <el-progress
                    :percentage="systemResources.cpu || 0"
                    :color="getResourceColor(systemResources.cpu)"
                    :show-text="false"
                    :stroke-width="8"
                  />
                </div>
                <div class="resource-item">
                  <div class="resource-info">
                    <span class="resource-label">内存使用率</span>
                    <span class="resource-value">{{ systemResources.memory || 0 }}%</span>
                  </div>
                  <el-progress
                    :percentage="systemResources.memory || 0"
                    :color="getResourceColor(systemResources.memory)"
                    :show-text="false"
                    :stroke-width="8"
                  />
                </div>
                <div class="resource-item">
                  <div class="resource-info">
                    <span class="resource-label">运行时间</span>
                    <span class="resource-value">{{ systemInfo.uptime }}</span>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <!-- Agent Status -->
          <div class="content-card">
            <div class="card-header">
              <h3>
                <el-icon><User /></el-icon>
                Agent 状态
              </h3>
              <el-button text @click="refreshAgents">
                <el-icon><Refresh /></el-icon>
              </el-button>
            </div>
            <div class="card-content">
              <div class="agent-status">
                <div class="agent-summary">
                  <div class="summary-item">
                    <div class="summary-value">{{ agentStats.total }}</div>
                    <div class="summary-label">总数量</div>
                  </div>
                  <div class="summary-item">
                    <div class="summary-value success">{{ agentStats.active }}</div>
                    <div class="summary-label">活跃中</div>
                  </div>
                </div>
                <div v-if="agentStats.total > 0" class="agent-list">
                  <div
                    v-for="agent in recentAgents"
                    :key="agent.id"
                    class="agent-item"
                  >
                    <div class="agent-status-indicator" :class="agent.status"></div>
                    <div class="agent-name">{{ agent.name }}</div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted, nextTick, watch } from 'vue'
import { useAppStore } from '@/stores/app'
import { useSystemStore } from '@/stores/system'
import { useServicesStore } from '@/stores/services'
import { useToolsStore } from '@/stores/tools'
import { useToolExecutionStore } from '@/stores/toolExecution'
import { api } from '@/api'
import { ElMessage } from 'element-plus'
import {
  CircleCheck, Connection, Tools, Refresh, Clock, TrendCharts,
  Star, Monitor, User, Plus, VideoPlay, FolderOpened, Box
} from '@element-plus/icons-vue'
import ErrorState from '@/components/common/ErrorState.vue'
import * as echarts from 'echarts'

// Store
const appStore = useAppStore()
const systemStore = useSystemStore()
const servicesStore = useServicesStore()
const toolsStore = useToolsStore()
const toolExecutionStore = useToolExecutionStore()

// Reactive Data
const isLoading = ref(true)
const hasError = ref(false)
const errorType = ref('network')
const errorTitle = ref('')
const errorDescription = ref('')
const errorDetails = ref('')
const showErrorDetails = ref(false)

const chartPeriod = ref('today')
const usageChart = ref(null)
let chartInstance = null

const systemResources = ref({
  cpu: 0,
  memory: 0
})

const systemInfo = ref({
  uptime: '00:00:00'
})


// Computed Properties
const greeting = computed(() => {
  const hour = new Date().getHours()
  if (hour < 6) return '凌晨好'
  if (hour < 9) return '早上好'
  if (hour < 12) return '上午好'
  if (hour < 14) return '中午好'
  if (hour < 17) return '下午好'
  if (hour < 19) return '傍晚好'
  return '晚上好'
})

const systemStatus = computed(() => systemStore.systemStatus)

const serviceStats = computed(() => ({
  total: servicesStore.services.length,
  healthy: servicesStore.healthyServices.length,
  remote: servicesStore.remoteServices.length,
  local: servicesStore.localServices.length
}))

const serviceDistribution = computed(() => {
  const total = serviceStats.value.total
  if (total === 0) return []

  return [
    {
      label: '健康服务',
      count: serviceStats.value.healthy,
      percentage: (serviceStats.value.healthy / total) * 100,
      color: 'var(--success-color)'
    },
    {
      label: '远程服务',
      count: serviceStats.value.remote,
      percentage: (serviceStats.value.remote / total) * 100,
      color: 'var(--primary-color)'
    },
    {
      label: '本地服务',
      count: serviceStats.value.local,
      percentage: (serviceStats.value.local / total) * 100,
      color: 'var(--info-color)'
    }
  ]
})

const toolStats = computed(() => ({
  available: toolsStore.availableTools.length,
  todayCalls: toolExecutionStore.todayStats.total
}))

const agentStats = ref({
  total: 0,
  active: 0
})

const popularTools = computed(() => toolExecutionStore.popularTools.slice(0, 5))

const recentActivities = ref([])
const recentAgents = ref([])

// Methods
const navigateTo = (path) => {
  // Using router push
  window.location.hash = path
}

const refreshAll = async () => {
  isLoading.value = true
  try {
    await Promise.all([
      systemStore.refreshAllData(),
      servicesStore.fetchServices(true),
      toolsStore.fetchTools(true),
      toolExecutionStore.fetchToolRecords(50, true)
    ])
    await fetchAgentData()
    await fetchSystemResources()
    ElMessage.success('数据刷新成功')
  } catch (error) {
    ElMessage.error('数据刷新失败')
  } finally {
    isLoading.value = false
  }
}

// Quick Actions
const quickActions = [
  {
    key: 'add-service',
    label: '添加服务',
    type: 'primary',
    icon: Plus,
    handler: () => navigateTo('/services/add')
  },
  {
    key: 'execute-tool',
    label: '执行工具',
    type: 'success',
    icon: VideoPlay,
    handler: () => navigateTo('/tools/execute')
  },
  {
    key: 'create-agent',
    label: '创建 Agent',
    type: 'info',
    icon: FolderOpened,
    handler: () => navigateTo('/agents/list')
  },
  {
    key: 'refresh-all',
    label: '刷新全部',
    type: 'warning',
    icon: Refresh,
    handler: refreshAll
  }
]

const refreshServices = async () => {
  try {
    await servicesStore.fetchServices(true)
    ElMessage.success('服务数据已更新')
  } catch (error) {
    ElMessage.error('服务数据更新失败')
  }
}

const refreshTools = async () => {
  try {
    await toolsStore.fetchTools(true)
    await toolExecutionStore.fetchToolRecords(50, true)
    ElMessage.success('工具数据已更新')
  } catch (error) {
    ElMessage.error('工具数据更新失败')
  }
}

const refreshActivity = async () => {
  try {
    await loadRecentActivities()
    ElMessage.success('活动记录已更新')
  } catch (error) {
    ElMessage.error('活动记录更新失败')
  }
}

const refreshResources = async () => {
  try {
    await fetchSystemResources()
    ElMessage.success('系统资源已更新')
  } catch (error) {
    ElMessage.error('系统资源更新失败')
  }
}

const refreshAgents = async () => {
  try {
    await fetchAgentData()
    ElMessage.success('Agent状态已更新')
  } catch (error) {
    ElMessage.error('Agent状态更新失败')
  }
}

const fetchAgentData = async () => {
  try {
    const response = await api.store.listAllAgents()
    const agents = response.data?.data?.agents || []
    agentStats.value = {
      total: agents.length,
      active: agents.filter(a => a.status === 'active' || a.status === 'healthy').length
    }
    recentAgents.value = agents.slice(0, 5)
  } catch (error) {
    console.error('获取Agent数据失败:', error)
  }
}

const fetchSystemResources = async () => {
  try {
    const response = await api.store.getSystemResources()
    if (response.data?.data) {
      systemResources.value = response.data.data
    }
  } catch (error) {
    console.error('获取系统资源失败:', error)
  }
}

const loadRecentActivities = async () => {
  try {
    // Get recent tool executions as activities
    const records = await toolExecutionStore.fetchToolRecords(10)
    // fetchToolRecords returns an object with executions array, not directly an array
    const executions = records.executions || []
    recentActivities.value = executions.map((record, index) => ({
      id: index,
      type: record.success ? 'success' : 'error',
      icon: record.success ? 'CircleCheck' : 'CircleClose',
      title: `执行工具: ${record.tool_name}`,
      timestamp: record.execution_time || new Date()
    }))
  } catch (error) {
    console.error('加载活动记录失败:', error)
  }
}

const formatTime = (timestamp) => {
  const date = new Date(timestamp)
  const now = new Date()
  const diff = now - date
  
  if (diff < 60000) return '刚刚'
  if (diff < 3600000) return `${Math.floor(diff / 60000)} 分钟前`
  if (diff < 86400000) return `${Math.floor(diff / 3600000)} 小时前`
  
  return date.toLocaleDateString('zh-CN')
}

const getResourceColor = (percentage) => {
  if (percentage < 50) return 'var(--success-color)'
  if (percentage < 80) return 'var(--warning-color)'
  return 'var(--danger-color)'
}

// Chart Management
const initChart = () => {
  if (!usageChart.value) return
  
  chartInstance = echarts.init(usageChart.value)
  updateChartData()
}

const updateChartData = async () => {
  if (!chartInstance) return
  
  try {
    const data = await getChartData(chartPeriod.value)
    const option = {
      tooltip: {
        trigger: 'axis',
        axisPointer: {
          type: 'shadow'
        }
      },
      grid: {
        left: '3%',
        right: '4%',
        bottom: '3%',
        containLabel: true
      },
      xAxis: {
        type: 'category',
        data: data.map(item => item.time),
        axisTick: {
          alignWithLabel: true
        }
      },
      yAxis: {
        type: 'value'
      },
      series: [
        {
          name: '工具调用次数',
          type: 'bar',
          barWidth: '60%',
          data: data.map(item => item.count),
          itemStyle: {
            color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [
              { offset: 0, color: 'var(--primary-color)' },
              { offset: 1, color: 'var(--primary-light)' }
            ])
          }
        }
      ]
    }
    
    chartInstance.setOption(option)
  } catch (error) {
    console.error('更新图表数据失败:', error)
  }
}

const getChartData = async (period) => {
  // Mock data based on period
  const periods = {
    today: Array.from({ length: 24 }, (_, i) => ({
      time: `${i}:00`,
      count: Math.floor(Math.random() * 20)
    })),
    week: Array.from({ length: 7 }, (_, i) => ({
      time: ['日', '一', '二', '三', '四', '五', '六'][i],
      count: Math.floor(Math.random() * 100)
    })),
    month: Array.from({ length: 30 }, (_, i) => ({
      time: `${i + 1}`,
      count: Math.floor(Math.random() * 200)
    }))
  }
  
  return periods[period] || periods.today
}

// Watch chart period changes
watch(chartPeriod, () => {
  updateChartData()
})

// Error Handling
const handleError = (error) => {
  hasError.value = true
  
  if (error.code === 'ECONNREFUSED' || error.code === 'ERR_NETWORK') {
    errorType.value = 'network'
    errorTitle.value = '无法连接到后端服务'
    errorDescription.value = '请检查后端服务是否正常运行，或稍后重试'
  } else {
    errorType.value = 'unknown'
    errorTitle.value = '加载失败'
    errorDescription.value = '数据加载失败，请稍后重试'
  }
}

const handleRetry = async () => {
  hasError.value = false
  await loadDashboardData()
}

// Load Dashboard Data
const loadDashboardData = async () => {
  isLoading.value = true
  try {
    await Promise.all([
      systemStore.fetchSystemStatus(),
      servicesStore.fetchServices(true),
      toolsStore.fetchTools(true),
      toolExecutionStore.fetchToolRecords(50, true)
    ])
    
    await Promise.all([
      fetchAgentData(),
      fetchSystemResources(),
      loadRecentActivities()
    ])
    
    // Initialize chart after data is loaded
    await nextTick()
    initChart()
    
  } catch (error) {
    handleError(error)
  } finally {
    isLoading.value = false
  }
}

// Lifecycle
onMounted(async () => {
  await loadDashboardData()
  
  // Update uptime every second
  setInterval(() => {
    updateUptime()
  }, 1000)
  
  // Auto refresh every 5 minutes
  setInterval(() => {
    refreshAll()
  }, 300000)
})

onUnmounted(() => {
  if (chartInstance) {
    chartInstance.dispose()
  }
})

// Update uptime
const updateUptime = () => {
  const startTime = localStorage.getItem('mcpstore_session_start')
  if (startTime) {
    const uptime = Date.now() - parseInt(startTime)
    const hours = Math.floor(uptime / 3600000)
    const minutes = Math.floor((uptime % 3600000) / 60000)
    const seconds = Math.floor((uptime % 60000) / 1000)
    
    systemInfo.value.uptime = `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}`
  }
}
</script>

<style scoped>
.dashboard-advanced {
  padding: 24px;
  background-color: var(--bg-color-page);
  min-height: 100vh;
}

.loading-container {
  padding: 40px;
}

/* Hero Section */
.hero-section {
  background: linear-gradient(135deg, var(--primary-color) 0%, var(--primary-dark) 100%);
  border-radius: var(--border-radius-lg);
  padding: 32px;
  margin-bottom: 32px;
  color: var(--text-inverse);
  box-shadow: var(--shadow-lg);
}

.hero-content {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.hero-title .greeting {
  font-size: var(--font-size-4xl);
  font-weight: var(--font-weight-bold);
  margin: 0 0 8px 0;
}

.hero-title .subtitle {
  font-size: var(--font-size-lg);
  opacity: 0.9;
  margin: 0;
}

.hero-metrics {
  display: flex;
  gap: 32px;
}

.metric-card {
  display: flex;
  align-items: center;
  gap: 16px;
  background: rgba(255, 255, 255, 0.1);
  padding: 20px;
  border-radius: var(--border-radius-md);
  backdrop-filter: blur(10px);
  transition: var(--transition-base);
}

.metric-card:hover {
  background: rgba(255, 255, 255, 0.2);
  transform: translateY(-2px);
}

.metric-icon {
  width: 48px;
  height: 48px;
  border-radius: var(--border-radius-full);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 24px;
}

.metric-icon.success {
  background: var(--success-color);
}

.metric-icon.primary {
  background: var(--info-color);
}

.metric-icon.info {
  background: var(--warning-color);
}

.metric-content .metric-value {
  font-size: var(--font-size-xl);
  font-weight: var(--font-weight-bold);
  line-height: 1;
}

.metric-content .metric-label {
  font-size: var(--font-size-sm);
  opacity: 0.8;
  margin-top: 4px;
}

/* Quick Actions Bar */
.quick-actions-bar {
  margin-bottom: 32px;
}

.section-title h2 {
  font-size: var(--font-size-2xl);
  font-weight: var(--font-weight-semibold);
  margin: 0 0 16px 0;
  color: var(--text-primary);
}

.quick-actions {
  display: flex;
  gap: 16px;
  flex-wrap: wrap;
}

.action-btn {
  min-width: 120px;
}

/* Content Grid */
.content-grid {
  display: grid;
  grid-template-columns: 300px 1fr 300px;
  gap: 24px;
}

.content-card {
  background: var(--bg-color);
  border-radius: var(--border-radius-lg);
  box-shadow: var(--shadow-base);
  overflow: hidden;
  transition: var(--transition-base);
}

.content-card:hover {
  box-shadow: var(--shadow-md);
}

.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px;
  border-bottom: 1px solid var(--border-lighter);
}

.card-header h3 {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0;
  font-size: var(--font-size-lg);
  font-weight: var(--font-weight-semibold);
  color: var(--text-primary);
}

.card-content {
  padding: 20px;
}

/* Service Distribution */
.service-distribution {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.distribution-item {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.distribution-info {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.distribution-info .label {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
}

.distribution-info .value {
  font-weight: var(--font-weight-semibold);
  color: var(--text-primary);
}

.distribution-bar {
  height: 8px;
  background: var(--bg-color-secondary);
  border-radius: var(--border-radius-full);
  overflow: hidden;
}

.bar-fill {
  height: 100%;
  border-radius: var(--border-radius-full);
  transition: width var(--transition-normal) ease;
}

/* Activity List */
.activity-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
  max-height: 300px;
  overflow-y: auto;
}

.activity-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  border-radius: var(--border-radius-base);
  transition: var(--transition-fast);
}

.activity-item:hover {
  background: var(--bg-color-secondary);
}

.activity-icon {
  width: 32px;
  height: 32px;
  border-radius: var(--border-radius-full);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.activity-icon.success {
  background: var(--success-lighter);
  color: var(--success-color);
}

.activity-icon.error {
  background: var(--danger-lighter);
  color: var(--danger-color);
}

.activity-details {
  flex: 1;
  min-width: 0;
}

.activity-title {
  font-weight: var(--font-weight-medium);
  color: var(--text-primary);
  margin-bottom: 2px;
}

.activity-time {
  font-size: var(--font-size-xs);
  color: var(--text-secondary);
}

/* Chart Card */
.chart-card .card-header {
  padding: 20px 20px 0;
}

.chart-controls {
  margin-bottom: 16px;
}

.chart-container {
  height: 300px;
}

/* Popular Tools */
.popular-tools {
  display: flex;
  flex-direction: column;
  gap: 12px;
  max-height: 300px;
  overflow-y: auto;
}

.tool-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  border-radius: var(--border-radius-base);
  transition: var(--transition-fast);
}

.tool-item:hover {
  background: var(--bg-color-secondary);
}

.tool-rank {
  width: 28px;
  height: 28px;
  border-radius: var(--border-radius-full);
  background: var(--primary-lighter);
  color: var(--primary-color);
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: var(--font-weight-bold);
  font-size: var(--font-size-sm);
  flex-shrink: 0;
}

.tool-info {
  flex: 1;
  min-width: 0;
}

.tool-name {
  font-weight: var(--font-weight-medium);
  color: var(--text-primary);
  margin-bottom: 2px;
}

.tool-service {
  font-size: var(--font-size-xs);
  color: var(--text-secondary);
}

.tool-stats {
  display: flex;
  gap: 16px;
}

.stat {
  text-align: center;
}

.stat-value {
  display: block;
  font-weight: var(--font-weight-semibold);
  color: var(--text-primary);
}

.stat-label {
  font-size: var(--font-size-xs);
  color: var(--text-secondary);
}

/* Resource Metrics */
.resource-metrics {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.resource-item {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.resource-info {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.resource-label {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
}

.resource-value {
  font-weight: var(--font-weight-semibold);
  color: var(--text-primary);
}

/* Agent Status */
.agent-status {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.agent-summary {
  display: flex;
  justify-content: space-around;
  padding: 20px;
  background: var(--bg-color-secondary);
  border-radius: var(--border-radius-md);
}

.summary-item {
  text-align: center;
}

.summary-value {
  font-size: var(--font-size-2xl);
  font-weight: var(--font-weight-bold);
  margin-bottom: 4px;
}

.summary-value.success {
  color: var(--success-color);
}

.summary-label {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
}

.agent-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.agent-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px;
  border-radius: var(--border-radius-sm);
  transition: var(--transition-fast);
}

.agent-item:hover {
  background: var(--bg-color-secondary);
}

.agent-status-indicator {
  width: 8px;
  height: 8px;
  border-radius: var(--border-radius-full);
  flex-shrink: 0;
}

.agent-status-indicator.active,
.agent-status-indicator.healthy {
  background: var(--success-color);
}

.agent-status-indicator.inactive {
  background: var(--text-placeholder);
}

.agent-name {
  font-size: var(--font-size-sm);
  color: var(--text-regular);
}

/* Empty State */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px 20px;
  color: var(--text-secondary);
  gap: 8px;
}

.empty-state .el-icon {
  font-size: 32px;
  opacity: 0.5;
}

/* Responsive Design */
@media (max-width: 1400px) {
  .content-grid {
    grid-template-columns: 1fr 1fr;
  }
  
  .right-column {
    grid-column: span 2;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 24px;
  }
}

@media (max-width: 1024px) {
  .hero-content {
    flex-direction: column;
    gap: 24px;
    text-align: center;
  }
  
  .hero-metrics {
    width: 100%;
    justify-content: space-around;
  }
  
  .content-grid {
    grid-template-columns: 1fr;
  }
  
  .right-column {
    grid-column: span 1;
    grid-template-columns: 1fr;
  }
}

@media (max-width: 768px) {
  .dashboard-advanced {
    padding: 16px;
  }
  
  .hero-section {
    padding: 20px;
  }
  
  .hero-title .greeting {
    font-size: var(--font-size-3xl);
  }
  
  .quick-actions {
    flex-direction: column;
  }
  
  .action-btn {
    width: 100%;
  }
}

/* Dark Mode Adaptations */
.dark .metric-card {
  background: rgba(255, 255, 255, 0.05);
}

.dark .metric-card:hover {
  background: rgba(255, 255, 255, 0.1);
}

.dark .tool-rank {
  background: var(--primary-lightest);
  color: var(--primary-light);
}
</style>