<template>
  <div class="dashboard page-container page-container--wide">
    <div v-if="isLoading" v-loading="isLoading" class="dashboard-loading"></div>
    <div v-else class="dashboard-content content-stack">
      <!-- KPI 区域 -->
      <el-row :gutter="12" class="dashboard-kpi-row">
        <el-col :span="4">
          <el-card shadow="hover" class="kpi-card">
            <div class="kpi">
              <div class="kpi-sub">Orchestrator status</div>
              <div class="kpi-row">
                <div class="kpi-value">{{ orchestratorLabel }}</div>
                <el-tag :type="systemStatus.running ? 'success' : 'danger'" effect="plain" size="small">{{ systemStatus.running ? 'running' : 'stopped' }}</el-tag>
              </div>
            </div>
          </el-card>
        </el-col>
        <el-col :span="2">
          <el-card shadow="hover" class="kpi-card">
            <div class="kpi">
              <div class="kpi-sub">Services / healthy</div>
              <div class="kpi-value">{{ totalServices }} / {{ healthyServices }}</div>
            </div>
          </el-card>
        </el-col>
        <el-col :span="2">
          <el-card shadow="hover" class="kpi-card">
            <div class="kpi">
              <div class="kpi-sub">Total tools</div>
              <div class="kpi-value">{{ totalTools }}</div>
            </div>
          </el-card>
        </el-col>
        <el-col :span="2">
          <el-card shadow="hover" class="kpi-card">
            <div class="kpi">
              <div class="kpi-sub">Tool calls (today)</div>
              <div class="kpi-value">{{ todayToolCalls }}</div>
            </div>
          </el-card>
        </el-col>
        <el-col :span="2">
          <el-card shadow="hover" class="kpi-card">
            <div class="kpi">
              <div class="kpi-sub">Tool calls (last 7 days)</div>
              <div class="kpi-value">{{ weekToolCalls }}</div>
            </div>
          </el-card>
        </el-col>
        <el-col :span="2">
          <el-card shadow="hover" class="kpi-card">
            <div class="kpi">
              <div class="kpi-sub">STDIO services</div>
              <div class="kpi-value">{{ stdioCount }}</div>
            </div>
          </el-card>
        </el-col>
        <el-col :span="2">
          <el-card shadow="hover" class="kpi-card">
            <div class="kpi">
              <div class="kpi-sub">SSE services</div>
              <div class="kpi-value">{{ sseCount }}</div>
            </div>
          </el-card>
        </el-col>
        <el-col :span="2">
          <el-card shadow="hover" class="kpi-card">
            <div class="kpi">
              <div class="kpi-sub">Streamable HTTP services</div>
              <div class="kpi-value">{{ streamHttpCount }}</div>
            </div>
          </el-card>
        </el-col>
        <el-col :span="2">
          <el-card shadow="hover" class="kpi-card">
            <div class="kpi">
              <div class="kpi-sub">API uptime</div>
              <div class="kpi-value small">{{ uptimeLabel }}</div>
            </div>
          </el-card>
        </el-col>
        <el-col :span="2">
          <el-card shadow="hover" class="kpi-card">
            <div class="kpi">
              <div class="kpi-sub">Agents count</div>
              <div class="kpi-value">{{ agentCountDisplay }}</div>
            </div>
          </el-card>
        </el-col>
        <el-col :span="2">
          <el-card shadow="hover" class="kpi-card">
            <div class="kpi">
              <div class="kpi-sub">Hub services count</div>
              <div class="kpi-value">{{ hubCountDisplay }}</div>
            </div>
          </el-card>
        </el-col>
      </el-row>

      <el-row :gutter="16" class="dashboard-main-row">
        <!-- 左：服务表格 + 工具表格 -->
        <el-col :span="14" class="dashboard-main-col">
          <el-card shadow="hover" class="dashboard-card dashboard-card--list">
            <template #header>
              <div class="section-header dashboard-card__header">
                <span class="section-title section-title--sm">服务列表</span>
                <div class="section-actions dashboard-card__filters">
                  <el-input v-model="serviceSearch" size="small" placeholder="搜索服务..." clearable class="dashboard-filter-input" />
                  <el-select v-model="statusFilter" size="small" placeholder="状态" class="dashboard-filter-select">
                    <el-option label="全部" value="all" />
                    <el-option label="健康" value="healthy" />
                    <el-option label="异常" value="unhealthy" />
                  </el-select>
                </div>
              </div>
            </template>
            <div class="dashboard-card__body">
              <el-table :data="filteredServices" size="small" :border="false" v-loading="tableLoading" height="100%">
              <el-table-column prop="name" label="名称" min-width="180" />
              <el-table-column prop="type" label="类型" width="140">
                <template #default="{ row }">
                  <el-tag size="small" type="info">{{ row.type || (row.command ? 'local' : 'remote') }}</el-tag>
                </template>
              </el-table-column>
              <el-table-column prop="status" label="状态" width="120">
                <template #default="{ row }">
                  <el-tag size="small" :type="row.status === 'healthy' ? 'success' : 'danger'">{{ row.status || 'unknown' }}</el-tag>
                </template>
              </el-table-column>
              <el-table-column prop="tools_count" label="工具数" width="100" />
              <el-table-column prop="client_id" label="ClientId" min-width="220" show-overflow-tooltip />
              <el-table-column label="最近变更" width="180">
                <template #default="{ row }">{{ formatLastChange(row.name) }}</template>
              </el-table-column>
              </el-table>
            </div>
          </el-card>

          <el-card shadow="hover" class="dashboard-card dashboard-card--list">
            <template #header>
              <div class="section-header dashboard-card__header">
                <span class="section-title section-title--sm">工具列表</span>
              </div>
            </template>
            <div class="dashboard-card__body">
              <el-table :data="toolsStore.tools" size="small" :border="false" height="100%" v-loading="tableLoading">
                <el-table-column prop="name" label="名称" min-width="220" />
                <el-table-column label="所属服务" width="160">
                  <template #default="{ row }">{{ row.service || row.service_name || '-' }}</template>
                </el-table-column>
                <el-table-column prop="description" label="描述" min-width="260" show-overflow-tooltip />
              </el-table>
            </div>
          </el-card>
        </el-col>

        <!-- 右：健康概览 + 工具分布图 -->
        <el-col :span="10" class="dashboard-side-col">
          <el-card shadow="hover" class="chart-card dashboard-card">
            <template #header>
              <div class="section-header dashboard-card__header">
                <span class="section-title section-title--sm">健康概览</span>
              </div>
            </template>
            <div class="chart-content">
              <div ref="healthPieRef" class="chart-canvas" />
            </div>
          </el-card>

          <el-card shadow="hover" class="chart-card dashboard-card">
            <template #header>
              <div class="section-header dashboard-card__header">
                <span class="section-title section-title--sm">工具分布（按服务）</span>
              </div>
            </template>
            <div class="chart-content">
              <div ref="toolsBarRef" class="chart-canvas" />
            </div>
          </el-card>
        </el-col>
      </el-row>
      <!-- 无刷新按钮：轮询由环境变量控制 -->
    </div>
  </div>
  </template>

<script setup>
import { ref, computed, onMounted, onUnmounted, onActivated } from 'vue'
import { useSystemStore } from '@/stores/system'
import { useServicesStore } from '@/stores/services'
import { useToolsStore } from '@/stores/tools'
import * as echarts from 'echarts'
import { api } from '@/api'

const systemStore = useSystemStore()
const servicesStore = useServicesStore()
const toolsStore = useToolsStore()

const isLoading = ref(true)
const refreshing = ref(false)
const tableLoading = ref(false)
let timer = null
const SECTION_HEIGHT = 260

// 轮询配置来自环境变量
const POLL_ENABLED = import.meta.env.VITE_DASHBOARD_POLLING_ENABLED === 'true'
const POLL_INTERVAL = Number(import.meta.env.VITE_DASHBOARD_POLL_INTERVAL_MS) || 60000

// 搜索/筛选
const serviceSearch = ref('')
const statusFilter = ref('all')

// 计算指标
const orchestratorLabel = computed(() => systemStore.healthStatus?.orchestrator_status || 'unknown')
const totalServices = computed(() => servicesStore.services.length)
const healthyServices = computed(() => servicesStore.healthyServices.length)
const totalTools = computed(() => toolsStore.tools.length)
const systemStatus = computed(() => systemStore.systemStatus)

// 过滤后的服务表
const filteredServices = computed(() => {
  const q = serviceSearch.value.trim().toLowerCase()
  const filter = statusFilter.value
  return servicesStore.services.filter(s => {
    const okText = !q || s.name?.toLowerCase().includes(q) || s.url?.toLowerCase().includes(q)
    const okStatus = filter === 'all' || (filter === 'healthy' ? s.status === 'healthy' : s.status !== 'healthy')
    return okText && okStatus
  })
})

// ECharts refs
const healthPieRef = ref(null)
const toolsBarRef = ref(null)
let healthPie, toolsBar

function renderHealthPie() {
  if (!healthPieRef.value) return
  if (!healthPie) healthPie = echarts.init(healthPieRef.value)
  const healthy = healthyServices.value
  const total = totalServices.value
  const unhealthy = Math.max(0, total - healthy)
  healthPie.setOption({
    tooltip: { trigger: 'item' },
    series: [{
      type: 'pie', radius: ['45%', '70%'], avoidLabelOverlap: false,
      label: { show: true, formatter: '{b}: {c} ({d}%)' },
      data: [
        { value: healthy, name: 'Healthy' },
        { value: unhealthy, name: 'Unhealthy' }
      ]
    }]
  })
}

function renderToolsBar() {
  if (!toolsBarRef.value) return
  if (!toolsBar) toolsBar = echarts.init(toolsBarRef.value)
  const serviceNames = servicesStore.services.map(s => s.name)
  const counts = servicesStore.services.map(s => Number(s.tools_count || 0))
  toolsBar.setOption({
    grid: { left: 40, right: 10, top: 20, bottom: 40 },
    xAxis: { type: 'category', data: serviceNames, axisLabel: { rotate: 30, interval: 0 } },
    yAxis: { type: 'value' },
    tooltip: { trigger: 'axis' },
    series: [{ type: 'bar', data: counts, itemStyle: { color: '#409EFF' } }]
  })
}

function resizeCharts() {
  healthPie?.resize()
  toolsBar?.resize()
}

function formatDateTime(ts) {
  if (!ts) return '-'
  const d = new Date(ts)
  if (Number.isNaN(d.getTime())) return '-'
  return d.toLocaleString('zh-CN')
}

function formatLastChange(serviceName) {
  const hs = Array.isArray(systemStore.healthStatus?.services) ? systemStore.healthStatus.services : []
  const item = hs.find(s => s.name === serviceName)
  return item?.last_state_change ? formatDateTime(item.last_state_change) : '-'
}

// KPI：工具调用统计
const todayToolCalls = ref(0)
const weekToolCalls = ref(0)

// KPI：服务类型统计
const serviceTypeCounts = computed(() => {
  const counts = { stdio: 0, sse: 0, streamable_http: 0, hub: 0 }
  servicesStore.services.forEach(s => {
    const t = (s.type || '').toLowerCase()
    if (t in counts) counts[t]++
  })
  return counts
})
const stdioCount = computed(() => serviceTypeCounts.value.stdio)
const sseCount = computed(() => serviceTypeCounts.value.sse)
const streamHttpCount = computed(() => serviceTypeCounts.value.streamable_http)

// KPI：运行时间
const uptimeSeconds = ref(null)
const uptimeLabel = computed(() => {
  if (!uptimeSeconds.value && uptimeSeconds.value !== 0) return '-'
  const s = Number(uptimeSeconds.value || 0)
  const d = Math.floor(s / 86400)
  const h = Math.floor((s % 86400) / 3600)
  const m = Math.floor((s % 3600) / 60)
  if (d > 0) return `${d}天 ${h}小时`
  if (h > 0) return `${h}小时 ${m}分`
  return `${m}分`
})

// KPI：Agent 数量 / Hub 数量（占位显示）
const agentCountDisplay = computed(() => {
  // TODO: 接入 agents 拉取后切换为实际数量
  return '—'
})
const hubCountDisplay = computed(() => {
  // TODO: 基于服务标记/类型识别 Hub 服务
  return '—'
})

async function loadDashboardData() {
  if (!isLoading.value) refreshing.value = true
  tableLoading.value = true
  try {
    await Promise.all([
      systemStore.fetchSystemStatus(),
      servicesStore.fetchServices(true),
      toolsStore.fetchTools(true)
    ])
    // 工具调用记录（用于今日/七天统计）
    try {
      const records = await toolsStore.getToolRecords(500, true)
      const executions = Array.isArray(records?.executions) ? records.executions : []
      const now = Date.now()
      const startOfToday = new Date()
      startOfToday.setHours(0, 0, 0, 0)
      const tsToday = startOfToday.getTime()
      const ts7 = now - 7 * 24 * 3600 * 1000
      const getTs = (e) => {
        const t = (e && (e.timestamp ?? e.execution_time ?? e.executed_at ?? e.time))
        if (typeof t === 'number') {
          // 兼容秒级与毫秒级时间戳
          return t > 1e12 ? t : t * 1000
        }
        const d = new Date(t)
        return Number.isNaN(d.getTime()) ? 0 : d.getTime()
      }
      todayToolCalls.value = executions.filter(e => getTs(e) >= tsToday).length
      weekToolCalls.value = executions.filter(e => getTs(e) >= ts7).length
    } catch (e) {
      todayToolCalls.value = 0
      weekToolCalls.value = 0
    }

    // 运行时间（来自 /for_store/health 简要统计）
    try {
      const stats = await api.store.getStats()
      uptimeSeconds.value = Number(stats?.uptime_seconds || 0)
    } catch (e) {
      uptimeSeconds.value = null
    }
    // 渲染图表
    setTimeout(() => {
      renderHealthPie()
      renderToolsBar()
      resizeCharts()
    }, 50)
  } catch (e) {
    console.error('加载仪表盘失败:', e)
  } finally {
    isLoading.value = false
    refreshing.value = false
    tableLoading.value = false
  }
}

function startTimer() {
  clearTimer()
  if (POLL_ENABLED) {
    timer = setInterval(() => {
      loadDashboardData()
    }, POLL_INTERVAL)
  }
}

function clearTimer() {
  if (timer) {
    clearInterval(timer)
    timer = null
  }
}

onMounted(async () => {
  window.addEventListener('resize', resizeCharts)
  await loadDashboardData() // 首次进入拉取
  startTimer()
})

onActivated(async () => {
  // 每次重新进入页面，立即查询
  await loadDashboardData()
})

onUnmounted(() => {
  window.removeEventListener('resize', resizeCharts)
  clearTimer()
  healthPie && healthPie.dispose()
  toolsBar && toolsBar.dispose()
})
</script>

<style scoped>
/* 页面容器：扩大可视宽度，左右保留比例空隙 */
.dashboard {
  width: 92%;
  margin: 0 auto;
  max-width: none;
}

/* 仪表盘整体布局 */
.dashboard-loading {
  min-height: 420px;
}

.dashboard-content {
  gap: 24px;
}

.dashboard-kpi-row {
  margin-bottom: 4px;
}

.dashboard-main-row {
  align-items: stretch;
}

.dashboard-main-col,
.dashboard-side-col {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

/* KPI 卡片 */
.kpi-card { height: 92px; display: flex; align-items: center; }
.kpi { width: 100%; display: flex; flex-direction: column; gap: 4px; }
.kpi-sub { font-size: 12px; color: var(--el-text-color-secondary); }
.kpi-value { font-size: 20px; font-weight: 600; line-height: 1; }
.kpi-value.small { font-size: 16px; }
.kpi-row { display: flex; align-items: center; gap: 8px; }

/* 主体卡片布局 */
.dashboard-card {
  display: flex;
  flex-direction: column;
  height: 360px;
}

.dashboard-card__header {
  width: 100%;
}

.section-title--sm {
  font-size: 18px;
}

.dashboard-card__filters {
  gap: 8px;
}

.dashboard-filter-input {
  width: 220px;
}

.dashboard-filter-select {
  width: 120px;
}

.dashboard-card__body {
  flex: 1;
  min-height: 0;
}

/* 图表区域 */
.chart-card {
  display: flex;
  flex-direction: column;
}

.chart-card :deep(.el-card__body) {
  flex: 1;
  display: flex;
  flex-direction: column;
}

.chart-content {
  flex: 1;
  min-height: 0;
  display: flex;
}

.chart-canvas {
  flex: 1;
  width: 100%;
  height: 100%;
}

@include respond-to(md) {
  .dashboard-filter-input,
  .dashboard-filter-select {
    width: 100%;
  }

  .dashboard-card__filters {
    width: 100%;
    justify-content: flex-start;
  }
}

@include respond-to(sm) {
  .dashboard-content {
    gap: 20px;
  }

  .dashboard-main-col,
  .dashboard-side-col {
    gap: 12px;
  }

  .dashboard-card {
    height: auto;
    min-height: 300px;
  }

  .section-title--sm {
    font-size: 16px;
  }
}
</style>
