<template>
  <div class="dashboard-container">
    <!-- Header -->
    <header class="dashboard-header">
      <div class="header-content">
        <h1 class="page-title">
          Dashboard
        </h1>
        <p class="page-subtitle">
          System performance overview
        </p>
      </div>
      <div class="header-actions">
        <span class="uptime-badge">
          API Uptime: {{ uptimeLabel }}
        </span>
      </div>
    </header>

    <!-- KPI Metrics: 纯数字展示 -->
    <div class="kpi-grid">
      <StatCard
        title="Orchestrator"
        :value="systemStatus.running ? 'Active' : 'Stopped'"
        :icon="Monitor"
        :description="systemStatus.running ? 'System operational' : 'System halted'"
        :class="['kpi-card', systemStatus.running ? 'status-active' : 'status-stopped']"
      />
      
      <StatCard
        title="Services Health"
        :value="healthyServices"
        unit="active"
        :icon="Connection"
        :description="`${totalServices} total services`"
        class="kpi-card"
      />
      
      <StatCard
        title="Tools Available"
        :value="totalTools"
        unit="fns"
        :icon="Tools"
        :trend="5" 
        class="kpi-card"
      />
      
      <StatCard
        title="Daily Invocations"
        :value="todayToolCalls"
        unit="calls"
        :icon="DataAnalysis"
        :trend="12"
        class="kpi-card"
      />
    </div>

    <!-- Main Content Grid -->
    <div class="main-layout">
      <!-- Left Column: Lists -->
      <div class="content-column left-col">
        <!-- Services List -->
        <section class="panel-section">
          <div class="panel-header">
            <h3 class="panel-title">
              Services
            </h3>
            <div class="panel-controls">
              <input 
                v-model="serviceSearch" 
                class="atom-input search-input" 
                placeholder="Search services..."
              >
              <select
                v-model="statusFilter"
                class="atom-input filter-select"
              >
                <option value="all">
                  All
                </option>
                <option value="healthy">
                  Healthy
                </option>
                <option value="unhealthy">
                  Issues
                </option>
              </select>
            </div>
          </div>
          
          <div class="panel-body table-container">
            <el-table
              :data="filteredServices"
              class="atom-table"
              :show-header="true"
              size="small"
            >
              <el-table-column
                prop="name"
                label="SERVICE"
                min-width="160"
              >
                <template #default="{ row }">
                  <div class="service-name-cell">
                    <div
                      class="status-indicator"
                      :class="row.status === 'healthy' ? 'is-healthy' : 'is-issue'"
                    />
                    <div class="name-wrapper">
                      <span class="primary-text">{{ row.name }}</span>
                      <span class="secondary-text">{{ row.type || 'remote' }}</span>
                    </div>
                  </div>
                </template>
              </el-table-column>
              
              <el-table-column
                label="STATUS"
                width="100"
              >
                <template #default="{ row }">
                  <span
                    class="status-text"
                    :class="row.status === 'healthy' ? 'text-regular' : 'text-danger'"
                  >
                    {{ row.status }}
                  </span>
                </template>
              </el-table-column>

              <el-table-column
                prop="tools_count"
                label="TOOLS"
                width="80"
                align="right"
              >
                <template #default="{ row }">
                  <span class="mono-number">{{ row.tools_count || 0 }}</span>
                </template>
              </el-table-column>
              
              <el-table-column
                label="ACTIVE"
                width="120"
                align="right"
              >
                <template #default="{ row }">
                  <span class="timestamp-text">{{ formatLastChange(row.name) }}</span>
                </template>
              </el-table-column>
            </el-table>
          </div>
        </section>

        <!-- Top Tools List -->
        <section class="panel-section">
          <div class="panel-header">
            <h3 class="panel-title">
              Top Tools
            </h3>
          </div>
          <div class="panel-body table-container">
            <el-table
              :data="toolsStore.tools.slice(0, 5)"
              class="atom-table"
              :show-header="true"
              size="small"
            >
              <el-table-column
                prop="name"
                label="TOOL NAME"
                min-width="180"
              >
                <template #default="{ row }">
                  <span class="primary-text">{{ row.name }}</span>
                </template>
              </el-table-column>
              <el-table-column
                prop="service"
                label="SERVICE"
                width="140"
              >
                <template #default="{ row }">
                  <span class="secondary-text">{{ row.service || '-' }}</span>
                </template>
              </el-table-column>
              <el-table-column
                prop="description"
                label="DESCRIPTION"
                min-width="200"
              >
                <template #default="{ row }">
                  <span class="secondary-text truncate">{{ row.description }}</span>
                </template>
              </el-table-column>
            </el-table>
          </div>
          <div class="panel-footer">
            <span class="link-text">View all {{ toolsStore.tools.length }} tools &rarr;</span>
          </div>
        </section>
      </div>

      <!-- Right Column: Charts & Info -->
      <div class="content-column right-col">
        <!-- Health Chart -->
        <section class="panel-section">
          <div class="panel-header">
            <h3 class="panel-title">
              System Health
            </h3>
          </div>
          <div class="panel-body chart-wrapper">
            <div
              ref="healthPieRef"
              class="chart-canvas"
            />
          </div>
          <div class="info-list">
            <div class="info-item">
              <span>Environment</span>
              <span class="mono-val">Production</span>
            </div>
            <div class="info-item">
              <span>API Version</span>
              <span class="mono-val">v0.6.0</span>
            </div>
          </div>
        </section>

        <!-- Distribution Chart -->
        <section class="panel-section">
          <div class="panel-header">
            <h3 class="panel-title">
              Tool Distribution
            </h3>
          </div>
          <div class="panel-body chart-wrapper">
            <div
              ref="toolsBarRef"
              class="chart-canvas"
            />
          </div>
        </section>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted, onActivated } from 'vue'
import { useSystemStore } from '@/stores/system'
import { useServicesStore } from '@/stores/services'
import { useToolsStore } from '@/stores/tools'
import { Monitor, Connection, Tools, DataAnalysis } from '@element-plus/icons-vue'
import StatCard from '@/components/common/StatCard.vue'
import * as echarts from 'echarts'

const systemStore = useSystemStore()
const servicesStore = useServicesStore()
const toolsStore = useToolsStore()

// Config
const serviceSearch = ref('')
const statusFilter = ref('all')
const uptimeLabel = ref('4d 12h')
const todayToolCalls = ref(124)

// Metrics
const totalServices = computed(() => servicesStore.services.length)
const healthyServices = computed(() => servicesStore.healthyServices.length)
const totalTools = computed(() => toolsStore.tools.length)
const systemStatus = computed(() => systemStore.systemStatus)

const filteredServices = computed(() => {
  const q = serviceSearch.value.trim().toLowerCase()
  const filter = statusFilter.value
  return servicesStore.services.filter(s => {
    const okText = !q || s.name?.toLowerCase().includes(q)
    const okStatus = filter === 'all' || (filter === 'healthy' ? s.status === 'healthy' : s.status !== 'healthy')
    return okText && okStatus
  })
})

const formatLastChange = () => '2m ago'

// Charts
const healthPieRef = ref(null)
const toolsBarRef = ref(null)
let healthPie, toolsBar

const chartTheme = {
  color: ['#111827', '#E5E7EB', '#9CA3AF', '#6B7280'],
  textStyle: { fontFamily: 'Inter, sans-serif' }
}

function renderCharts() {
  if (healthPieRef.value) {
    if (!healthPie) healthPie = echarts.init(healthPieRef.value)
    healthPie.setOption({
      ...chartTheme,
      tooltip: { trigger: 'item' },
      series: [{
        type: 'pie',
        radius: ['60%', '80%'],
        center: ['50%', '50%'],
        avoidLabelOverlap: false,
        label: { show: false },
        data: [
          { value: healthyServices.value, name: 'Healthy', itemStyle: { color: '#10B981' } },
          { value: Math.max(0, totalServices.value - healthyServices.value), name: 'Issues', itemStyle: { color: '#EF4444' } }
        ]
      }]
    })
  }

  if (toolsBarRef.value) {
    if (!toolsBar) toolsBar = echarts.init(toolsBarRef.value)
    const topServices = [...servicesStore.services].sort((a,b) => b.tools_count - a.tools_count).slice(0, 5)
    toolsBar.setOption({
      ...chartTheme,
      grid: { left: 0, right: 0, top: 10, bottom: 20, containLabel: true },
      xAxis: { show: false },
      yAxis: { 
        type: 'category', 
        data: topServices.map(s => s.name),
        axisLine: { show: false },
        axisTick: { show: false },
        axisLabel: { color: '#6B7280', fontSize: 11 }
      },
      series: [{
        type: 'bar',
        data: topServices.map(s => s.tools_count),
        barWidth: 8,
        itemStyle: { borderRadius: 4, color: '#111827' },
        showBackground: true,
        backgroundStyle: { color: '#F3F4F6', borderRadius: 4 }
      }]
    })
  }
}

onMounted(async () => {
  await Promise.all([systemStore.fetchSystemStatus(), servicesStore.fetchServices(), toolsStore.fetchTools()])
  renderCharts()
  window.addEventListener('resize', () => { healthPie?.resize(); toolsBar?.resize() })
})
</script>

<style lang="scss" scoped>
.dashboard-container {
  max-width: 1440px;
  margin: 0 auto;
  padding: 20px; // Reduced from 32px
  width: 100%;
}

// Header
.dashboard-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  margin-bottom: 20px; // Reduced from 32px
  padding-bottom: 12px; // Reduced from 16px
  border-bottom: 1px solid var(--border-color);
}

.page-title {
  font-size: 20px;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 2px; // Reduced
}

.page-subtitle {
  font-size: 13px; // Slightly smaller
  color: var(--text-secondary);
}

.uptime-badge {
  font-size: 12px;
  font-family: var(--font-mono);
  color: var(--text-secondary);
  background: var(--bg-surface);
  border: 1px solid var(--border-color);
  padding: 2px 8px; // Compact padding
  border-radius: 4px;
}

// KPI Grid
.kpi-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 16px; // Reduced from 24px
  margin-bottom: 20px; // Reduced from 32px

  @media (max-width: 1200px) { grid-template-columns: repeat(2, 1fr); }
  @media (max-width: 640px) { grid-template-columns: 1fr; }
}

.kpi-card {
  height: 100%;
  min-height: 100px; // Reduced from 120px
  
  &.status-active { border-left: 3px solid var(--color-success) !important; }
  &.status-stopped { border-left: 3px solid var(--color-danger) !important; }
}

// Main Layout
.main-layout {
  display: grid;
  grid-template-columns: 2.5fr 1fr;
  gap: 20px; // Reduced from 32px

  @media (max-width: 1024px) {
    grid-template-columns: 1fr;
  }
}

.content-column {
  display: flex;
  flex-direction: column;
  gap: 20px; // Reduced from 32px
}

// Panel Sections
.panel-section {
  display: flex;
  flex-direction: column;
  gap: 12px; // Reduced from 16px
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 4px; // Reduced from 8px
}

.panel-title {
  font-size: 13px; // Slightly smaller
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-secondary);
}

.panel-controls {
  display: flex;
  gap: 8px;
}

.panel-body {
  background: var(--bg-surface);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.panel-footer {
  padding-top: 6px; // Reduced
  text-align: center;
}

.link-text {
  font-size: 12px; // Smaller
  color: var(--text-secondary);
  cursor: pointer;
  transition: color 0.2s;
  
  &:hover { color: var(--text-primary); }
}

// Inputs
.atom-input {
  border: 1px solid var(--border-color);
  background: var(--bg-surface);
  padding: 4px 10px; // More compact
  border-radius: 6px;
  font-size: 13px;
  color: var(--text-primary);
  
  &:focus { outline: none; border-color: var(--text-secondary); }
}

.search-input { width: 180px; } // Slightly narrower
.filter-select { width: 90px; }

// Table Customization
.table-container {
  max-height: 400px; // Reduced max height
  overflow-y: auto;
}

:deep(.atom-table) {
  --el-table-border-color: var(--border-color);
  --el-table-header-bg-color: transparent;
  --el-table-row-hover-bg-color: var(--bg-hover);
  background: transparent;

  th.el-table__cell {
    background: transparent !important;
    border-bottom: 1px solid var(--border-color) !important;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    letter-spacing: 0.05em;
    padding: 8px 12px; // Reduced padding
  }

  td.el-table__cell {
    border-bottom: 1px solid var(--border-color) !important;
    padding: 8px 12px; // Reduced padding
  }
  
  .el-table__inner-wrapper::before { display: none; }
}

// Cell Content
.service-name-cell {
  display: flex;
  align-items: center;
  gap: 10px; // Reduced
}

.status-indicator {
  width: 6px; // Smaller dot
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
  
  &.is-healthy { background-color: var(--color-success); }
  &.is-issue { background-color: var(--color-danger); }
}

.name-wrapper {
  display: flex;
  flex-direction: column;
}

.primary-text {
  font-size: 13px; // Smaller
  font-weight: 500;
  color: var(--text-primary);
}

.secondary-text {
  font-size: 11px; // Smaller
  color: var(--text-secondary);
}

.status-text {
  font-size: 12px;
  text-transform: capitalize;
}

.mono-number {
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--text-secondary);
}

.timestamp-text {
  font-size: 11px;
  color: var(--text-placeholder);
}

.truncate {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

// Charts
.chart-wrapper {
  padding: 12px; // Reduced
  height: 200px; // Reduced height
  display: flex;
  align-items: center;
  justify-content: center;
}

.chart-canvas {
  width: 100%;
  height: 100%;
}

// Info List
.info-list {
  margin-top: 12px; // Reduced
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  background: var(--bg-surface);
  padding: 0 12px; // Reduced
}

.info-item {
  display: flex;
  justify-content: space-between;
  padding: 10px 0; // Reduced
  font-size: 12px; // Smaller
  border-bottom: 1px solid var(--border-color);
  color: var(--text-secondary);
  
  &:last-child { border-bottom: none; }
}

.mono-val {
  font-family: var(--font-mono);
  color: var(--text-primary);
}
</style>