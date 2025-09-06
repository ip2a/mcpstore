<template>
  <div class="service-analytics">
    <!-- Page Header -->
    <div class="page-header">
      <div class="header-content">
        <div class="header-title">
          <h1>服务使用分析</h1>
          <p class="subtitle">深入了解服务使用情况和性能指标</p>
        </div>
        <div class="header-actions">
          <el-button
            type="primary"
            :icon="Refresh"
            @click="refreshAnalytics"
            :loading="loading"
          >
            刷新数据
          </el-button>
          <el-dropdown @command="handleExportCommand" trigger="click">
            <el-button type="success">
              导出报告
              <el-icon class="el-icon--right"><ArrowDown /></el-icon>
            </el-button>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item command="pdf">PDF报告</el-dropdown-item>
                <el-dropdown-item command="excel">Excel数据</el-dropdown-item>
                <el-dropdown-item command="json">JSON数据</el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </div>
      </div>
    </div>

    <!-- Time Range Selector -->
    <el-card class="time-range-card">
      <el-row :gutter="20" align="middle">
        <el-col :span="20">
          <el-radio-group v-model="timeRange" @change="handleTimeRangeChange">
            <el-radio-button label="day">今日</el-radio-button>
            <el-radio-button label="week">本周</el-radio-button>
            <el-radio-button label="month">本月</el-radio-button>
            <el-radio-button label="quarter">本季度</el-radio-button>
            <el-radio-button label="custom">自定义</el-radio-button>
          </el-radio-group>
        </el-col>
        <el-col :span="4" v-if="timeRange === 'custom'">
          <el-date-picker
            v-model="customDateRange"
            type="daterange"
            range-separator="至"
            start-placeholder="开始日期"
            end-placeholder="结束日期"
            format="YYYY-MM-DD"
            value-format="YYYY-MM-DD"
            @change="handleCustomDateChange"
          />
        </el-col>
      </el-row>
    </el-card>

    <!-- Overview Metrics -->
    <div class="metrics-grid">
      <el-card v-for="metric in overviewMetrics" :key="metric.key" class="metric-card">
        <div class="metric-content">
          <div class="metric-icon" :class="metric.type">
            <el-icon><component :is="metric.icon" /></el-icon>
          </div>
          <div class="metric-info">
            <div class="metric-value">{{ metric.value }}</div>
            <div class="metric-label">{{ metric.label }}</div>
            <div v-if="metric.change !== null" class="metric-change" :class="metric.change > 0 ? 'positive' : 'negative'">
              <el-icon><component :is="metric.change > 0 ? ArrowUp : ArrowDown" /></el-icon>
              {{ Math.abs(metric.change) }}%
            </div>
          </div>
        </div>
      </el-card>
    </div>

    <!-- Charts Section -->
    <el-row :gutter="24">
      <!-- Service Usage Trend -->
      <el-col :span="16">
        <el-card class="chart-card">
          <template #header>
            <div class="card-header">
              <span>服务使用趋势</span>
              <el-radio-group v-model="usageChartType" size="small">
                <el-radio-button label="line">折线图</el-radio-button>
                <el-radio-button label="bar">柱状图</el-radio-button>
              </el-radio-group>
            </div>
          </template>
          <div ref="usageChartRef" class="chart-container"></div>
        </el-card>
      </el-col>

      <!-- Service Distribution -->
      <el-col :span="8">
        <el-card class="chart-card">
          <template #header>
            <span>服务类型分布</span>
          </template>
          <div ref="distributionChartRef" class="chart-container"></div>
        </el-card>
      </el-col>
    </el-row>

    <!-- Top Services Table -->
    <el-card class="services-table-card">
      <template #header>
        <div class="card-header">
          <span>服务使用排行</span>
          <el-input
            v-model="searchQuery"
            placeholder="搜索服务..."
            style="width: 200px"
            :prefix-icon="Search"
            clearable
          />
        </div>
      </template>
      
      <el-table
        :data="filteredServices"
        style="width: 100%"
        :default-sort="{ prop: 'usageCount', order: 'descending' }"
      >
        <el-table-column prop="rank" label="排名" width="80" align="center">
          <template #default="{ row }">
            <div class="rank-badge" :class="getRankClass(row.rank)">
              {{ row.rank }}
            </div>
          </template>
        </el-table-column>
        <el-table-column prop="name" label="服务名称" min-width="150">
          <template #default="{ row }">
            <div class="service-info">
              <el-icon><Connection /></el-icon>
              <span>{{ row.name }}</span>
              <el-tag v-if="row.type === 'local'" size="small" type="success">本地</el-tag>
              <el-tag v-else size="small" type="primary">远程</el-tag>
            </div>
          </template>
        </el-table-column>
        <el-table-column prop="usageCount" label="使用次数" width="120" sortable>
          <template #default="{ row }">
            <div class="usage-count">
              {{ row.usageCount.toLocaleString() }}
            </div>
          </template>
        </el-table-column>
        <el-table-column prop="avgResponseTime" label="平均响应时间" width="140" sortable>
          <template #default="{ row }">
            <div class="response-time">
              {{ row.avgResponseTime }}ms
              <el-progress
                :percentage="Math.min(row.avgResponseTime / 1000 * 100, 100)"
                :show-text="false"
                :color="getResponseTimeColor(row.avgResponseTime)"
                :stroke-width="4"
              />
            </div>
          </template>
        </el-table-column>
        <el-table-column prop="successRate" label="成功率" width="120" sortable>
          <template #default="{ row }">
            <div class="success-rate">
              {{ row.successRate }}%
              <el-progress
                :percentage="row.successRate"
                :color="getSuccessRateColor(row.successRate)"
                :stroke-width="4"
              />
            </div>
          </template>
        </el-table-column>
        <el-table-column prop="trend" label="趋势" width="100" align="center">
          <template #default="{ row }">
            <div class="trend-indicator" :class="row.trend">
              <el-icon><component :is="row.trend === 'up' ? ArrowUp : ArrowDown" /></el-icon>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="120" fixed="right">
          <template #default="{ row }">
            <el-button text size="small" @click="viewServiceDetail(row)">
              查看详情
            </el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- Tool Execution Analysis -->
    <el-row :gutter="24" class="mt-24">
      <!-- Popular Tools -->
      <el-col :span="12">
        <el-card class="chart-card">
          <template #header>
            <span>热门工具使用排行</span>
          </template>
          <div ref="toolsChartRef" class="chart-container"></div>
        </el-card>
      </el-col>

      <!-- Error Analysis -->
      <el-col :span="12">
        <el-card class="chart-card">
          <template #header>
            <span>错误类型分析</span>
          </template>
          <div ref="errorsChartRef" class="chart-container"></div>
        </el-card>
      </el-col>
    </el-row>

    <!-- Insights Section -->
    <el-card class="insights-card">
      <template #header>
        <div class="card-header">
          <span>使用洞察</span>
          <el-badge :value="insights.length" type="info" />
        </div>
      </template>
      
      <el-timeline>
        <el-timeline-item
          v-for="(insight, index) in insights"
          :key="index"
          :type="insight.type"
          :timestamp="insight.timestamp"
          :icon="getInsightIcon(insight.type)"
        >
          <div class="insight-content">
            <h4>{{ insight.title }}</h4>
            <p>{{ insight.description }}</p>
            <div v-if="insight.action" class="insight-action">
              <el-button size="small" type="primary" @click="handleInsightAction(insight)">
                {{ insight.action }}
              </el-button>
            </div>
          </div>
        </el-timeline-item>
      </el-timeline>
    </el-card>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onBeforeUnmount, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { useSystemStore } from '@/stores/system'
import { useToolExecutionStore } from '@/stores/toolExecution'
import * as echarts from 'echarts'
import {
  Refresh, ArrowDown, ArrowUp, Search, Connection,
  DataBoard, Tools, WarningFilled, InfoFilled, TrendCharts
} from '@element-plus/icons-vue'

const router = useRouter()
const systemStore = useSystemStore()
const toolExecutionStore = useToolExecutionStore()

// Reactive Data
const loading = ref(false)
const timeRange = ref('week')
const customDateRange = ref([])
const searchQuery = ref('')
const usageChartType = ref('line')

// Chart references
const usageChartRef = ref(null)
const distributionChartRef = ref(null)
const toolsChartRef = ref(null)
const errorsChartRef = ref(null)

// Chart instances
let usageChart = null
let distributionChart = null
let toolsChart = null
let errorsChart = null

// Overview metrics
const overviewMetrics = ref([
  { key: 'totalRequests', label: '总请求数', value: '0', icon: 'DataBoard', type: 'primary', change: 12.5 },
  { key: 'avgResponseTime', label: '平均响应时间', value: '0ms', icon: 'TrendCharts', type: 'warning', change: -5.2 },
  { key: 'successRate', label: '成功率', value: '0%', icon: 'SuccessFilled', type: 'success', change: 2.1 },
  { key: 'activeServices', label: '活跃服务', value: '0', icon: 'Connection', type: 'info', change: 0 }
])

// Service usage data
const servicesData = ref([
  {
    rank: 1,
    name: 'mcp-server-1',
    type: 'local',
    usageCount: 15420,
    avgResponseTime: 245,
    successRate: 98.5,
    trend: 'up'
  },
  {
    rank: 2,
    name: 'remote-mcp-service',
    type: 'remote',
    usageCount: 12350,
    avgResponseTime: 412,
    successRate: 96.2,
    trend: 'up'
  },
  {
    rank: 3,
    name: 'data-processor',
    type: 'local',
    usageCount: 9870,
    avgResponseTime: 178,
    successRate: 99.1,
    trend: 'down'
  }
])

// Insights data
const insights = ref([
  {
    type: 'success',
    timestamp: '2024-01-15 14:30',
    title: '服务性能优化建议',
    description: 'remote-mcp-service 服务的响应时间在过去一周内提升了15%，建议继续保持当前的配置。',
    action: '查看详情'
  },
  {
    type: 'warning',
    timestamp: '2024-01-15 10:15',
    title: '异常使用模式检测',
    description: '检测到 mcp-server-1 在凌晨时段有异常的请求高峰，建议检查是否存在异常调用。',
    action: '分析日志'
  },
  {
    type: 'info',
    timestamp: '2024-01-14 16:45',
    title: '工具使用趋势',
    description: '文本处理工具的使用量在本月增长了35%，成为最受欢迎的工具类型。',
    action: '查看趋势'
  }
])

// Computed Properties
const filteredServices = computed(() => {
  if (!searchQuery.value) return servicesData.value
  const query = searchQuery.value.toLowerCase()
  return servicesData.value.filter(service =>
    service.name.toLowerCase().includes(query)
  )
})

// Methods
const refreshAnalytics = async () => {
  loading.value = true
  try {
    // Fetch real data from stores
    await systemStore.fetchServices()
    await systemStore.fetchTools()
    await toolExecutionStore.fetchToolRecords(100, true)
    
    // Update metrics
    updateMetrics()
    
    // Update charts
    await nextTick()
    initCharts()
    
    ElMessage.success('数据已刷新')
  } catch (error) {
    console.error('Failed to refresh analytics:', error)
    ElMessage.error('刷新失败')
  } finally {
    loading.value = false
  }
}

const updateMetrics = () => {
  // Calculate metrics from real data
  const totalRequests = toolExecutionStore.executionHistory.length
  const successRequests = toolExecutionStore.executionHistory.filter(
    exec => exec.status === 'success'
  ).length
  const successRate = totalRequests > 0 ? (successRequests / totalRequests * 100).toFixed(1) : 0
  
  overviewMetrics.value = [
    { key: 'totalRequests', label: '总请求数', value: totalRequests.toLocaleString(), icon: 'DataBoard', type: 'primary', change: 12.5 },
    { key: 'avgResponseTime', label: '平均响应时间', value: '245ms', icon: 'TrendCharts', type: 'warning', change: -5.2 },
    { key: 'successRate', label: '成功率', value: `${successRate}%`, icon: 'SuccessFilled', type: 'success', change: 2.1 },
    { key: 'activeServices', label: '活跃服务', value: systemStore.services.length, icon: 'Connection', type: 'info', change: 0 }
  ]
}

const handleTimeRangeChange = (range) => {
  // Update analytics based on time range
  refreshAnalytics()
}

const handleCustomDateChange = () => {
  if (customDateRange.value && customDateRange.value.length === 2) {
    refreshAnalytics()
  }
}

const handleExportCommand = (command) => {
  ElMessage.success(`正在导出 ${command.toUpperCase()} 格式报告...`)
  // Implement export functionality
}

const getRankClass = (rank) => {
  if (rank === 1) return 'rank-1'
  if (rank === 2) return 'rank-2'
  if (rank === 3) return 'rank-3'
  return ''
}

const getResponseTimeColor = (time) => {
  if (time < 200) return '#67C23A'
  if (time < 500) return '#E6A23C'
  return '#F56C6C'
}

const getSuccessRateColor = (rate) => {
  if (rate >= 95) return '#67C23A'
  if (rate >= 90) return '#E6A23C'
  return '#F56C6C'
}

const getInsightIcon = (type) => {
  switch (type) {
    case 'success':
      return 'SuccessFilled'
    case 'warning':
      return 'WarningFilled'
    case 'info':
      return 'InfoFilled'
    default:
      return 'InfoFilled'
  }
}

const viewServiceDetail = (service) => {
  router.push(`/services/detail/${service.name}`)
}

const handleInsightAction = (insight) => {
  ElMessage.info(`执行操作: ${insight.action}`)
  // Implement insight action handling
}

// Chart initialization
const initCharts = () => {
  // Usage trend chart
  if (usageChartRef.value) {
    usageChart = echarts.init(usageChartRef.value)
    const usageOption = {
      tooltip: {
        trigger: 'axis',
        axisPointer: {
          type: 'shadow'
        }
      },
      legend: {
        data: ['请求数', '成功率']
      },
      grid: {
        left: '3%',
        right: '4%',
        bottom: '3%',
        containLabel: true
      },
      xAxis: {
        type: 'category',
        data: ['周一', '周二', '周三', '周四', '周五', '周六', '周日']
      },
      yAxis: [
        {
          type: 'value',
          name: '请求数',
          position: 'left'
        },
        {
          type: 'value',
          name: '成功率',
          min: 0,
          max: 100,
          position: 'right'
        }
      ],
      series: [
        {
          name: '请求数',
          type: usageChartType.value,
          data: [1200, 1900, 1500, 2100, 2300, 1800, 1600],
          itemStyle: {
            color: '#5B5FDE'
          }
        },
        {
          name: '成功率',
          type: 'line',
          yAxisIndex: 1,
          data: [98, 97, 98, 99, 98, 97, 98],
          itemStyle: {
            color: '#67C23A'
          }
        }
      ]
    }
    usageChart.setOption(usageOption)
  }

  // Distribution chart
  if (distributionChartRef.value) {
    distributionChart = echarts.init(distributionChartRef.value)
    const distributionOption = {
      tooltip: {
        trigger: 'item',
        formatter: '{a} <br/>{b}: {c} ({d}%)'
      },
      legend: {
        orient: 'vertical',
        left: 'left'
      },
      series: [
        {
          name: '服务类型',
          type: 'pie',
          radius: ['40%', '70%'],
          avoidLabelOverlap: false,
          itemStyle: {
            borderRadius: 10,
            borderColor: '#fff',
            borderWidth: 2
          },
          label: {
            show: false,
            position: 'center'
          },
          emphasis: {
            label: {
              show: true,
              fontSize: 20,
              fontWeight: 'bold'
            }
          },
          labelLine: {
            show: false
          },
          data: [
            { value: 7, name: '本地服务', itemStyle: { color: '#67C23A' } },
            { value: 5, name: '远程服务', itemStyle: { color: '#409EFF' } },
            { value: 3, name: '混合服务', itemStyle: { color: '#E6A23C' } }
          ]
        }
      ]
    }
    distributionChart.setOption(distributionOption)
  }

  // Tools chart
  if (toolsChartRef.value) {
    toolsChart = echarts.init(toolsChartRef.value)
    const toolsOption = {
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
        type: 'value'
      },
      yAxis: {
        type: 'category',
        data: ['文本处理', '数据转换', '文件操作', '网络请求', 'AI工具']
      },
      series: [
        {
          type: 'bar',
          data: [320, 280, 250, 200, 180],
          itemStyle: {
            color: new echarts.graphic.LinearGradient(0, 0, 1, 0, [
              { offset: 0, color: '#83bff6' },
              { offset: 0.5, color: '#188df0' },
              { offset: 1, color: '#188df0' }
            ])
          }
        }
      ]
    }
    toolsChart.setOption(toolsOption)
  }

  // Errors chart
  if (errorsChartRef.value) {
    errorsChart = echarts.init(errorsChartRef.value)
    const errorsOption = {
      tooltip: {
        trigger: 'item'
      },
      legend: {
        orient: 'vertical',
        left: 'left'
      },
      series: [
        {
          name: '错误类型',
          type: 'pie',
          radius: '50%',
          data: [
            { value: 35, name: '连接超时', itemStyle: { color: '#F56C6C' } },
            { value: 20, name: '参数错误', itemStyle: { color: '#E6A23C' } },
            { value: 15, name: '权限不足', itemStyle: { color: '#909399' } },
            { value: 10, name: '服务不可用', itemStyle: { color: '#606266' } },
            { value: 20, name: '其他错误', itemStyle: { color: '#C0C4CC' } }
          ],
          emphasis: {
            itemStyle: {
              shadowBlur: 10,
              shadowOffsetX: 0,
              shadowColor: 'rgba(0, 0, 0, 0.5)'
            }
          }
        }
      ]
    }
    errorsChart.setOption(errorsOption)
  }
}

// Handle window resize
const handleResize = () => {
  usageChart?.resize()
  distributionChart?.resize()
  toolsChart?.resize()
  errorsChart?.resize()
}

// Lifecycle
onMounted(async () => {
  await refreshAnalytics()
  
  // Add resize listener
  window.addEventListener('resize', handleResize)
})

// Cleanup
onBeforeUnmount(() => {
  window.removeEventListener('resize', handleResize)
  usageChart?.dispose()
  distributionChart?.dispose()
  toolsChart?.dispose()
  errorsChart?.dispose()
})
</script>

<style lang="scss" scoped>
.service-analytics {
  max-width: 1400px;
  margin: 0 auto;
}

/* Page Header */
.page-header {
  margin-bottom: 32px;
}

.header-content {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.header-title h1 {
  font-size: var(--font-size-4xl);
  font-weight: var(--font-weight-bold);
  margin: 0 0 8px 0;
  color: var(--text-primary);
}

.header-title .subtitle {
  font-size: var(--font-size-lg);
  color: var(--text-secondary);
  margin: 0;
}

.header-actions {
  display: flex;
  gap: 12px;
}

/* Time Range Card */
.time-range-card {
  margin-bottom: 24px;
}

/* Metrics Grid */
.metrics-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
  gap: 20px;
  margin-bottom: 32px;
}

.metric-card {
  transition: var(--transition-base);
  
  &:hover {
    transform: translateY(-2px);
    box-shadow: var(--shadow-md);
  }
}

.metric-content {
  display: flex;
  align-items: center;
  gap: 16px;
}

.metric-icon {
  width: 56px;
  height: 56px;
  border-radius: var(--border-radius-xl);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 24px;
  flex-shrink: 0;
  
  &.primary {
    background: var(--primary-lighter);
    color: var(--primary-color);
  }
  
  &.success {
    background: var(--success-lighter);
    color: var(--success-color);
  }
  
  &.warning {
    background: var(--warning-lighter);
    color: var(--warning-color);
  }
  
  &.info {
    background: var(--info-lighter);
    color: var(--info-color);
  }
}

.metric-info {
  flex: 1;
}

.metric-value {
  font-size: var(--font-size-2xl);
  font-weight: var(--font-weight-bold);
  color: var(--text-primary);
  line-height: 1.2;
}

.metric-label {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
  margin-top: 4px;
}

.metric-change {
  display: flex;
  align-items: center;
  gap: 2px;
  font-size: var(--font-size-xs);
  margin-top: 4px;
  
  &.positive {
    color: var(--success-color);
  }
  
  &.negative {
    color: var(--danger-color);
  }
}

/* Chart Cards */
.chart-card {
  margin-bottom: 24px;
  
  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  
  .chart-container {
    height: 300px;
  }
}

/* Services Table */
.services-table-card {
  margin-bottom: 24px;
}

.rank-badge {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-weight: var(--font-weight-bold);
  font-size: var(--font-size-sm);
  
  &.rank-1 {
    background: linear-gradient(135deg, #FFD700, #FFA500);
    color: white;
  }
  
  &.rank-2 {
    background: linear-gradient(135deg, #C0C0C0, #808080);
    color: white;
  }
  
  &.rank-3 {
    background: linear-gradient(135deg, #CD7F32, #8B4513);
    color: white;
  }
}

.service-info {
  display: flex;
  align-items: center;
  gap: 8px;
  
  .el-icon {
    color: var(--primary-color);
  }
}

.usage-count {
  font-weight: var(--font-weight-medium);
}

.response-time,
.success-rate {
  .el-progress {
    margin-top: 4px;
  }
}

.trend-indicator {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: 50%;
  
  &.up {
    background: var(--success-lighter);
    color: var(--success-color);
  }
  
  &.down {
    background: var(--danger-lighter);
    color: var(--danger-color);
  }
}

/* Insights Card */
.insights-card {
  .insight-content {
    h4 {
      margin: 0 0 8px 0;
      color: var(--text-primary);
    }
    
    p {
      margin: 0 0 12px 0;
      color: var(--text-regular);
    }
    
    .insight-action {
      margin-top: 8px;
    }
  }
}

/* Responsive Design */
@media (max-width: 768px) {
  .header-content {
    flex-direction: column;
    gap: 16px;
    align-items: flex-start;
  }
  
  .metrics-grid {
    grid-template-columns: 1fr;
  }
  
  .chart-container {
    height: 250px !important;
  }
}
</style>