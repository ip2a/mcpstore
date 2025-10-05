<template>
  <div class="services-page">
    <!-- 状态统计横幅和图表 -->
    <el-row :gutter="20" class="mb-3">
      <el-col :xs="24" :sm="12" :md="12">
        <ArtBasicBanner
          title="服务状态总览"
          :subtitle="headerSubtitle"
          :titleColor="'var(--el-text-color-primary)'"
          :subtitleColor="'var(--el-text-color-secondary)'"
          :backgroundColor="'var(--el-color-primary-light-9)'"
          :buttonConfig="{
            show: true,
            text: '刷新状态',
            color: 'var(--el-color-primary)',
            textColor: '#fff',
            radius: '6px'
          }"
          @buttonClick="refreshServices"
        />
      </el-col>
      <el-col :xs="12" :sm="6" :md="6">
        <div class="card art-custom-card mini-chart-card">
          <div class="chart-header">
            <div class="chart-value">{{ healthyCount }}</div>
            <div class="chart-label">健康服务</div>
          </div>
          <div class="chart-container-mini">
            <ArtRingChart
              :data="healthChartData"
              :radius="['50%', '80%']"
              :showLegend="false"
              height="120px"
            />
          </div>
        </div>
      </el-col>
      <el-col :xs="12" :sm="6" :md="6">
        <div class="card art-custom-card mini-chart-card">
          <div class="chart-header">
            <div class="chart-value">{{ totalServices }}</div>
            <div class="chart-label">服务总数</div>
          </div>
          <div class="chart-container-mini">
            <ArtBarChart
              :data="serviceTypeData"
              :xAxisData="serviceTypeLabels"
              :showLegend="false"
              height="120px"
              barWidth="60%"
            />
          </div>
        </div>
      </el-col>
    </el-row>

    <!-- 服务列表表格 -->
    <div class="card art-custom-card">
      <ArtTableHeader :loading="tableLoading" @refresh="refreshTable" :showSearchBar="showSearchBar" @update:showSearchBar="(v)=> (showSearchBar = v)">
        <template #left>
          <h3 class="table-title">服务列表</h3>
        </template>
        <template #right>
          <div class="table-actions">
            <el-button type="primary" @click="addService">
              添加服务
            </el-button>
          </div>
        </template>
      </ArtTableHeader>

      <ArtSearchBar
        v-if="showSearchBar"
        v-model="searchForm"
        :items="searchItems"
        :span="6"
        :showExpand="false"
        @search="applyFilters"
        @reset="resetFilters"
      />

      <ArtTable
        :data="filteredData"
        :columns="tableColumns"
        :loading="tableLoading"
        :pagination="pagination"
        @size-change="handleSizeChange"
        @current-change="handleCurrentChange"
        stripe
        border
      >
        <!-- 合并后的操作列：文字按钮，无图标 -->
        <template #actions="{ row }">
          <div style="display:flex; gap:8px; justify-content:flex-end;">
            <el-button link type="primary" @click="viewService(row)">详情</el-button>
            <el-button link type="warning" @click="restartService(row)">重启</el-button>
            <el-button link type="danger" @click="deleteService(row)">删除</el-button>
          </div>
        </template>
      </ArtTable>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, computed, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useRouter } from 'vue-router'

const router = useRouter()

// 响应式数据
const tableLoading = ref(false)
const tableData = ref<any[]>([])

// 筛选相关
const showSearchBar = ref(true)
const searchForm = reactive({ name: '', type: '', status: '' })
const searchItems = [
  { key: 'name', label: '名称', type: 'input', placeholder: '服务名关键字' },
  { key: 'type', label: '类型', type: 'select', options: [
    { label: '全部', value: '' },
    { label: 'streamable_http', value: 'streamable_http' },
    { label: 'stdio', value: 'stdio' },
    { label: 'url', value: 'url' }
  ] },
  { key: 'status', label: '状态', type: 'select', options: [
    { label: '全部', value: '' },
    { label: 'healthy', value: 'healthy' },
    { label: 'reconnecting', value: 'reconnecting' },
    { label: 'unhealthy', value: 'unhealthy' },
    { label: 'disconnected', value: 'disconnected' },
    { label: 'initializing', value: 'initializing' },
    { label: 'unknown', value: 'unknown' }
  ] }
]
const filteredData = computed(() => {
  return tableData.value.filter((s) => {
    const byName = !searchForm.name || s.name?.toLowerCase().includes(searchForm.name.toLowerCase())
    const byType = !searchForm.type || s.type === searchForm.type
    const byStatus = !searchForm.status || s.status === searchForm.status
    return byName && byType && byStatus
  })
})
const applyFilters = () => { /* 使用计算属性即可，这里保留钩子 */ }
const resetFilters = () => {
  searchForm.name = ''
  searchForm.type = ''
  searchForm.status = ''
  
}

// MCP 数据（原始）
const servicesResponse = ref<any>(null)

// MCP API
import { mcpApi } from '../../api/index'

// 载入服务数据
const loadServices = async () => {
  tableLoading.value = true
  try {
    const services = await mcpApi.listServices() // data 为数组
    servicesResponse.value = { data: services }

    // 转换到表格行（使用原始 type、status；拆分 url/command/args；新增 keepAlive）
    tableData.value = (services || []).map((s: any, idx: number) => {
      const url = s.url || ''
      const command = s.command || ''
      const argsText = Array.isArray(s.args) ? s.args.join(' ') : ''
      const lastCheck = s.last_check || '-'

      return {
        id: idx + 1,
        name: s.name,
        type: s.type || 'unknown',
        status: s.status || 'unknown',
        keepAlive: Boolean(s.keep_alive),
        url,
        command,
        args: argsText,
        lastCheck,
        lastCheckAgo: formatTimeAgoFromString(lastCheck),
        toolCount: s.tools_count ?? s.tool_count ?? 0
      }
    })

    // 更新分页
    pagination.total = tableData.value.length
  } catch (e) {
    ElMessage.error('获取服务列表失败')
    console.error(e)
  } finally {
    tableLoading.value = false
  }
}


// 分页配置
const pagination = reactive({
  currentPage: 1,
  pageSize: 10,
  total: 0,
  showSizeChanger: true,
  showQuickJumper: true,
  showTotal: true
})

// 表格列配置
const tableColumns = [
  { prop: 'id', label: '#', width: 60 },
  { prop: 'name', label: 'Name', minWidth: 160 },
  { prop: 'type', label: 'Type', width: 150 },
  { prop: 'status', label: 'Status', width: 120 },
  { prop: 'keepAlive', label: 'KeepAlive', width: 110 },
  { prop: 'url', label: 'URL', minWidth: 220 },
  { prop: 'command', label: 'Command', width: 100 },
  { prop: 'args', label: 'Args', minWidth: 220 },
  { prop: 'toolCount', label: 'Tools', width: 100 },
  { prop: 'actions', label: 'Actions', headerAlign: 'center', width: 160, useSlot: true, fixed: 'right' }
]

// 计算属性
const totalServices = computed(() => tableData.value.length)
const healthyCount = computed(() => tableData.value.filter(s => s.status === 'healthy').length)
// 移除了不再需要的百分比计算，因为改回自定义结构
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

// 横幅文案：当前共有 X 个服务，其中 Y 个健康，Z 个不健康。
const headerSubtitle = computed(() => {
  const total = totalServices.value
  const healthy = healthyCount.value
  const unhealthy = total - healthy
  return `当前共有 ${total} 个服务，其中 ${healthy} 个健康，${unhealthy} 个不健康。`
})

// 方法
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

// 显示“xx分钟前/小时前/天前”
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

const refreshServices = async () => {
  await loadServices()
  ElMessage.success('服务状态已刷新')
}

const refreshTable = async () => {
  await loadServices()
  ElMessage.success('表格数据已刷新')
}

const addService = () => {
  router.push('/add-service')
}

const viewService = (row: any) => {
  // 跳转到服务详情页面，可以通过路由参数传递服务信息
  router.push({ 
    path: '/services/detail', 
    query: { name: row.name }
  })
}

const editService = (row: any) => {
  // 跳转到编辑服务页面
  router.push({ 
    path: '/services/edit', 
    query: { name: row.name }
  })
}

const restartService = async (row: any) => {
  try {
    ElMessage.info(`正在重启服务: ${row.name}`)
    await mcpApi.restartService(row.name)
    ElMessage.success(`服务 ${row.name} 重启成功`)
    await loadServices()
  } catch (error: any) {
    ElMessage.error(`重启服务失败: ${error?.message || error}`)
  }
}

const toggleService = async (row: any) => {
  const action = row.status === 'running' ? '停止' : '启动'
  try {
    await ElMessageBox.confirm(`确定要${action}服务 "${row.name}" 吗？`, '确认操作', {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      type: 'warning'
    })
    
    ElMessage.info(`正在${action}服务: ${row.name}`)
    // TODO: 实现具体的启动/停止API调用
    // await serviceApi.toggleService(row.name, row.status === 'running' ? 'stop' : 'start')
    
    // 暂时更新本地状态
    row.status = row.status === 'running' ? 'stopped' : 'running'
    ElMessage.success(`服务已${action}`)
    
    // 重新加载数据以获取最新状态
    await loadServices()
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error(`${action}服务失败: ${error}`)
    }
  }
}

const deleteService = async (row: any) => {
  try {
    await ElMessageBox.confirm(`确定要删除服务 "${row.name}" 吗？此操作不可恢复。`, '确认删除', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning'
    })

    ElMessage.info(`正在删除服务: ${row.name}`)
    await mcpApi.deleteService(row.name)
    ElMessage.success('服务已删除')
    await loadServices()
  } catch (error: any) {
    if (error !== 'cancel') {
      ElMessage.error(`删除服务失败: ${error?.message || error}`)
    }
  }
}

const handleSizeChange = (size: number) => {
  pagination.pageSize = size
  pagination.currentPage = 1
}

const handleCurrentChange = (page: number) => {
  pagination.currentPage = page
}

onMounted(() => {
  loadServices()
})
</script>

<style lang="scss" scoped>
.services-page {
  padding: 20px;
  
  .mb-3 {
    margin-bottom: 20px;
  }
  
  .card,
  .art-custom-card {
    background: var(--el-bg-color);
    border: 1px solid var(--el-border-color-light);
    border-radius: 8px;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
    padding: 20px;
    margin-bottom: 20px;
    transition: all 0.3s ease;

    &:hover {
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    }

    .box-title {
      font-size: 18px;
      font-weight: 500;
      color: var(--el-text-color-primary);
      margin-bottom: 16px;
    }

    .subtitle {
      color: var(--el-text-color-secondary);
      font-size: 14px;
    }

    .text-success { color: var(--el-color-success); }
    .text-warning { color: var(--el-color-warning); }
    .text-info { color: var(--el-color-info); }
    .text-danger { color: var(--el-color-danger); }
  }
  
  .mini-chart-card {
    .chart-header {
      margin-bottom: 12px;
      
      .chart-value {
        font-size: 24px;
        font-weight: 600;
        color: var(--el-text-color-primary);
        margin-bottom: 4px;
      }
      
      .chart-label {
        font-size: 14px;
        color: var(--el-text-color-secondary);
      }
    }
    
    .chart-container-mini {
      height: 120px;
    }
  }
  
  .table-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 20px;
    
    .table-title {
      font-size: 18px;
      font-weight: 500;
      color: var(--el-text-color-primary);
      margin: 0;
    }
    
    .table-actions {
      display: flex;
      gap: 12px;
    }
  }
  
  // 响应式设计
  @media (max-width: 768px) {
    padding: 12px;
    
    .table-header {
      flex-direction: column;
      align-items: flex-start;
      gap: 12px;
      
      .table-actions {
        width: 100%;
        justify-content: flex-start;
      }
    }
    
    .mini-chart-card {
      margin-bottom: 12px;
      
      .chart-header .chart-value {
        font-size: 20px;
      }
      
      .chart-container-mini {
        height: 100px;
      }
    }
  }
}
</style>
