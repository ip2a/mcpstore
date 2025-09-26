<template>
  <div class="mcp-dashboard">
    <h1>MCP 仪表盘</h1>
    <p>测试页面 - 如果您看到这个，说明路由配置成功了</p>

    <el-row :gutter="20" class="mb-3">
      <el-col :sm="12" :md="6" v-for="card in statCards" :key="card.title">
        <el-card shadow="hover" class="stat-card">
          <div class="card-title">{{ card.title }}</div>
          <div class="card-value">{{ card.value }}</div>
          <div class="card-sub">{{ card.sub }}</div>
        </el-card>
      </el-col>
    </el-row>

    <el-row :gutter="20" class="mb-3">
      <el-col :sm="24" :md="12">
        <el-card shadow="hover" :loading="loading">
          <template #header>
            <div class="card-header">服务健康概览</div>
          </template>
          <el-space wrap>
            <el-tag type="success">Healthy: {{ healthCount.healthy }}</el-tag>
            <el-tag type="warning">Warning: {{ healthCount.warning }}</el-tag>
            <el-tag type="info">Reconnecting: {{ healthCount.reconnecting }}</el-tag>
            <el-tag type="danger">Unreachable: {{ healthCount.unreachable }}</el-tag>
          </el-space>
        </el-card>
      </el-col>
      <el-col :sm="24" :md="12">
        <el-card shadow="hover" :loading="loading">
          <template #header>
            <div class="card-header">系统资源</div>
          </template>
          <div class="resource">
            <div class="resource-item">
              <span>CPU 使用率</span>
              <el-progress :percentage="resources.cpu_usage ?? 0" />
            </div>
            <div class="resource-item">
              <span>内存使用率</span>
              <el-progress :percentage="resources.memory_usage ?? 0" status="success" />
            </div>
            <div class="resource-item">
              <span>磁盘使用率</span>
              <el-progress :percentage="resources.disk_usage ?? 0" status="warning" />
            </div>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <el-row :gutter="20">
      <el-col :sm="24" :md="24">
        <el-card shadow="hover" :loading="loading">
          <template #header>
            <div class="card-header">最近工具调用</div>
          </template>
          <el-table :data="toolRecords" border size="small">
            <el-table-column type="index" label="#" width="60" />
            <el-table-column prop="tool_name" label="工具" />
            <el-table-column prop="service_name" label="服务" width="220" />
            <el-table-column prop="status" label="状态" width="120">
              <template #default="{ row }">
                <el-tag :type="row.is_error ? 'danger' : 'success'">{{ row.is_error ? '失败' : '成功' }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="timestamp" label="时间" width="200" />
          </el-table>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, computed, ref } from 'vue'
import { mcpApi } from '../api'

const loading = ref(false)
const services = ref<string[]>([])
const tools = ref<any[]>([])
const agents = ref<any[]>([])
const healthCount = reactive({ healthy: 0, warning: 0, unreachable: 0, reconnecting: 0 })
const resources = reactive<{ cpu_usage?: number; memory_usage?: number; disk_usage?: number }>({})
const toolRecords = ref<any[]>([])

const statCards = computed(() => [
  { title: '服务总数', value: services.value.length, sub: '' },
  { title: '工具总数', value: tools.value.length, sub: '' },
  { title: 'Agent 数', value: agents.value.length, sub: '' },
  { title: '健康服务', value: healthCount.healthy, sub: `告警 ${healthCount.warning} · 断开 ${healthCount.unreachable}` }
])

async function refresh() {
  loading.value = true
  try {
    const [svc, tls, ags] = await Promise.all([
      mcpApi.listServices(),
      mcpApi.listTools(),
      mcpApi.listAllAgents().catch(() => [])
    ])
    services.value = Array.isArray(svc) ? svc : []
    tools.value = Array.isArray(tls) ? tls : []
    agents.value = Array.isArray(ags) ? ags : []

    // 健康数据
    const checks = await mcpApi.checkServices().catch(() => [])
    const counters = { healthy: 0, warning: 0, unreachable: 0, reconnecting: 0 }
    for (const item of checks as any[]) {
      const s = (item?.status || '').toLowerCase()
      if (s.includes('healthy')) counters.healthy++
      else if (s.includes('warning')) counters.warning++
      else if (s.includes('reconnecting')) counters.reconnecting++
      else counters.unreachable++
    }
    Object.assign(healthCount, counters)

    // 系统资源
    const res = await mcpApi.getSystemResources().catch(() => ({} as any))
    resources.cpu_usage = normalizePercent(res?.cpu_usage)
    resources.memory_usage = normalizePercent(res?.memory_usage)
    resources.disk_usage = normalizePercent(res?.disk_usage)

    // 工具记录
    toolRecords.value = await mcpApi.getToolRecords(10).catch(() => [])
  } finally {
    loading.value = false
  }
}

function normalizePercent(v: any) {
  if (v == null) return 0
  if (typeof v === 'number') {
    if (v <= 1) return Math.round(v * 100)
    return Math.max(0, Math.min(100, Math.round(v)))
  }
  const n = Number(v)
  if (!isNaN(n)) return normalizePercent(n)
  return 0
}

onMounted(refresh)
</script>

<style scoped>
.mcp-page { padding: 12px; }
.mb-3 { margin-bottom: 12px; }
.stat-card { min-height: 120px; }
.card-title { font-size: 14px; color: var(--el-text-color-secondary); }
.card-value { font-size: 28px; margin: 6px 0; }
.card-sub { font-size: 12px; color: var(--el-text-color-secondary); }
.resource .resource-item { margin-bottom: 12px; }
.card-header { font-weight: 600; }
</style>

