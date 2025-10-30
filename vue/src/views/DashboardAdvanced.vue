<template>
  <div class="dashboard">
    <!-- Loading State -->
    <div v-if="isLoading" v-loading="isLoading" style="min-height: 400px"></div>

    <!-- Dashboard Content -->
    <div v-else>
      <!-- Hero Section - 第一行横幅 -->
      <el-row :gutter="20" style="margin-bottom: 20px;">
        <el-col :span="24">
          <el-card shadow="hover" style="background: linear-gradient(135deg, #e8f3ff 0%, #d6ecff 100%); border: none;">
            <div style="display: flex; justify-content: space-between; align-items: center; color: var(--el-text-color-primary);">
              <div>
                <h1 style="margin: 0 0 8px 0; font-size: 28px; font-weight: 600;">{{ greeting }}</h1>
                <p style="margin: 0; font-size: 14px; opacity: 0.9;">系统运行状态概览</p>
              </div>
              <div style="display: flex; gap: 32px;">
                <div style="text-align: center;">
                  <div style="font-size: 28px; font-weight: 600;">{{ systemStatus.running ? '正常' : '异常' }}</div>
                  <div style="font-size: 12px; opacity: 0.8; margin-top: 4px;">系统状态</div>
                </div>
                <div style="text-align: center;">
                  <div style="font-size: 28px; font-weight: 600;">{{ serviceStats.healthy }}</div>
                  <div style="font-size: 12px; opacity: 0.8; margin-top: 4px;">健康服务</div>
                </div>
                <div style="text-align: center;">
                  <div style="font-size: 28px; font-weight: 600;">{{ toolStats.available }}</div>
                  <div style="font-size: 12px; opacity: 0.8; margin-top: 4px;">可用工具</div>
                </div>
              </div>
            </div>
          </el-card>
        </el-col>
      </el-row>

      <!-- 第二行 - 左折线图 + 右服务信息卡片 -->
      <el-row :gutter="20">
        <!-- 折线图 - 工具使用趋势 -->
        <el-col :span="16">
          <el-card shadow="hover">
            <template #header>
              <div style="display: flex; justify-content: space-between; align-items: center;">
                <span style="font-weight: bold; font-size: 16px;">
                  <el-icon style="vertical-align: middle; margin-right: 5px;"><TrendCharts /></el-icon>
                  工具使用趋势
                </span>
                <el-radio-group v-model="chartPeriod" size="small">
                  <el-radio-button label="today">今日</el-radio-button>
                  <el-radio-button label="week">本周</el-radio-button>
                  <el-radio-button label="month">本月</el-radio-button>
                </el-radio-group>
              </div>
            </template>
            <div ref="lineChart" style="width: 100%; height: 350px;"></div>
          </el-card>
        </el-col>

        <!-- 服务基本信息 -->
        <el-col :span="8">
          <el-card shadow="hover">
            <template #header>
              <span style="font-weight: bold; font-size: 16px;">服务概览</span>
            </template>
            <el-descriptions :column="1" size="small" border>
              <el-descriptions-item label="服务总数">{{ serviceBasic.total }}</el-descriptions-item>
              <el-descriptions-item label="健康服务">{{ serviceBasic.healthy }}</el-descriptions-item>
              <el-descriptions-item label="远程服务">{{ serviceBasic.remote }}</el-descriptions-item>
              <el-descriptions-item label="本地服务">{{ serviceBasic.local }}</el-descriptions-item>
            </el-descriptions>
          </el-card>
        </el-col>
      </el-row>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useSystemStore } from '@/stores/system'
import { useServicesStore } from '@/stores/services'
import { useToolsStore } from '@/stores/tools'
import { useToolExecutionStore } from '@/stores/toolExecution'
import { TrendCharts } from '@element-plus/icons-vue'
import { api } from '@/api'
import * as echarts from 'echarts'

// Stores
const systemStore = useSystemStore()
const servicesStore = useServicesStore()
const toolsStore = useToolsStore()
const toolExecutionStore = useToolExecutionStore()

// Reactive Data
const isLoading = ref(true)
const chartPeriod = ref('today')

// Chart Refs
const lineChart = ref(null)

let lineChartInstance = null

// Store/API derived summary
const serviceSummary = ref(null)

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

const toolStats = computed(() => ({
  available: toolsStore.availableTools.length,
  todayCalls: toolExecutionStore.todayStats.total
}))

// Chart Initialization
const initLineChart = () => {
  if (!lineChart.value) return
  lineChartInstance = echarts.init(lineChart.value)

  const option = {
    tooltip: {
      trigger: 'axis',
      formatter: (params) => (Array.isArray(params) && params[0]) ? params[0].value : ''
    },
    grid: {
      left: '3%',
      right: '4%',
      bottom: '3%',
      containLabel: true
    },
    xAxis: {
      type: 'category',
      boundaryGap: false,
      data: ['00:00', '04:00', '08:00', '12:00', '16:00', '20:00', '24:00']
    },
    yAxis: {
      type: 'value'
    },
    series: [{
      type: 'line',
      smooth: true,
      data: [12, 23, 45, 67, 89, 56, 34],
      areaStyle: {
        color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [{
          offset: 0,
          color: 'rgba(64, 158, 255, 0.5)'
        }, {
          offset: 1,
          color: 'rgba(64, 158, 255, 0.1)'
        }])
      },
      itemStyle: {
        color: '#409EFF'
      }
    }]
  }

  lineChartInstance.setOption(option)
}




// Computed basic service info (merge API stats and pinia stats)
const updateLineChart = () => {
  if (!lineChartInstance) return

  const dataMap = {
    today: {
      xData: ['00:00', '04:00', '08:00', '12:00', '16:00', '20:00', '24:00'],
      data: [12, 23, 45, 67, 89, 56, 34]
    },
    week: {
      xData: ['周一', '周二', '周三', '周四', '周五', '周六', '周日'],
      data: [120, 200, 150, 80, 70, 110, 130]
    },
    month: {
      xData: ['第1周', '第2周', '第3周', '第4周'],
      data: [820, 932, 901, 934]
    }
  }

  const chartData = dataMap[chartPeriod.value] || dataMap.today

  lineChartInstance.setOption({
    xAxis: {
      data: chartData.xData
    },
    series: [{
      data: chartData.data
    }]
  })
}

// Watch chart period changes
watch(chartPeriod, () => {
  updateLineChart()
})

// 计算服务基础信息（合并 API 与 Pinia）
const serviceBasic = computed(() => {
  const s = servicesStore
  const piniaStats = {
    total: s.stats.total || (s.services?.length || 0),
    healthy: s.healthyServices?.length || 0,
    remote: s.remoteServices?.length || 0,
    local: s.localServices?.length || 0
  }
  const apiStats = serviceSummary.value || {}
  return {
    total: apiStats.total_services ?? apiStats.services_total ?? piniaStats.total,
    healthy: apiStats.healthy_services ?? piniaStats.healthy,
    remote: apiStats.remote_services ?? piniaStats.remote,
    local: apiStats.local_services ?? piniaStats.local
  }
})

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

    try {
      const stats = await api.store.getStats()
      serviceSummary.value = stats || null
    } catch (e) {
      console.warn('获取统计信息失败，使用 Pinia 统计作为回退', e)
      serviceSummary.value = null
    }

    setTimeout(() => {
      initLineChart()
    }, 100)

  } catch (error) {
    console.error('加载数据失败:', error)
  } finally {
    isLoading.value = false
  }
}

// Lifecycle
onMounted(async () => {
  await loadDashboardData()
})

onUnmounted(() => {
  if (lineChartInstance) lineChartInstance.dispose()
})
</script>

<style scoped>
/* 使用 Element Plus 默认样式 */
</style>
