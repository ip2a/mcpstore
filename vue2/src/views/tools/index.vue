<template>
  <div class="tools-page art-full-height">
    <!-- 状态统计横幅和图表 -->
    <el-row :gutter="20" class="mb-3">
      <el-col :xs="24" :sm="12" :md="12">
        <ArtBasicBanner
          title="工具使用概览"
          :subtitle="headerSubtitle"
          :titleColor="'var(--el-text-color-primary)'"
          :subtitleColor="'var(--el-text-color-secondary)'"
          :backgroundColor="'var(--el-color-success-light-9)'"
          :buttonConfig="{
            show: true,
            text: '刷新数据',
            color: 'var(--el-color-success)',
            textColor: '#fff',
            radius: '6px'
          }"
          @buttonClick="refreshTools"
        />
      </el-col>
      <el-col :xs="12" :sm="6" :md="6">
        <div class="card art-custom-card mini-chart-card">
          <div class="chart-header">
            <div class="chart-value">{{ successRate }}%</div>
            <div class="chart-label">成功率</div>
          </div>
          <div class="chart-container-mini">
            <ArtRingChart
              :data="successChartData"
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
            <div class="chart-value">{{ totalTools }}</div>
            <div class="chart-label">工具总数</div>
          </div>
          <div class="chart-container-mini">
            <ArtBarChart
              :data="toolTypeData"
              :xAxisData="toolTypeLabels"
              :showLegend="false"
              height="120px"
              barWidth="60%"
            />
          </div>
        </div>
      </el-col>
    </el-row>

    <!-- 搜索筛选 -->
    <ArtSearchBar
      v-model="searchForm"
      :items="searchFormItems"
      @search="handleSearch"
      @reset="handleReset"
      class="mb-3"
    />

    <!-- 工具列表表格 -->
    <el-card class="art-table-card table-card-enhanced" shadow="hover">
      <ArtTableHeader :loading="tableLoading" @refresh="refreshTable">
        <template #left>
          <el-space wrap>
            <el-button type="primary" @click="batchExecute" :disabled="selectedRows.length === 0">
              批量执行
            </el-button>
            <el-button @click="exportTools">
              导出工具
            </el-button>
          </el-space>
        </template>
      </ArtTableHeader>

      <ArtTable
        :data="filteredTableData"
        :columns="tableColumns"
        :loading="tableLoading"
        :pagination="pagination"
        @selection-change="handleSelectionChange"
        @size-change="handleSizeChange"
        @current-change="handleCurrentChange"
        stripe
        border
      >
        <!-- 输入模式插槽 -->
        <template #has_schema="{ row }">
          <el-tag :type="row.has_schema ? 'success' : 'info'" size="small">
            {{ row.has_schema ? '有' : '无' }}
          </el-tag>
        </template>

        <!-- 操作插槽 -->
        <template #actions="{ row }">
          <el-button type="primary" size="small" @click="executeTool(row)">执行</el-button>
          <el-button type="info" size="small" @click="viewTool(row)">详情</el-button>
          <el-button type="warning" size="small" @click="testTool(row)">测试</el-button>
        </template>
      </ArtTable>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, computed, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { dashboardApi } from '@/mcp/api/dashboard'
import { useRouter } from 'vue-router'

const router = useRouter()
const headerSubtitle = computed(() => {
  return `当前共有 ${totalTools.value} 个工具，来源于 ${new Set(tableData.value.map(t => t.service_name)).size} 个服务。`
})


defineOptions({ name: 'ToolList' })

// 响应式数据
const tableLoading = ref(false)
const selectedRows = ref<any[]>([])

// 搜索表单
const searchForm = ref({
  serviceType: '',
  serviceName: '',
  toolName: ''
})

// 搜索表单配置
const searchFormItems = computed(() => [
  {
    key: 'serviceType',
    label: '服务类型',
    type: 'select',
    props: {
      placeholder: '请选择服务类型',
      clearable: true,
      options: [
        { label: '远程服务', value: 'remote' },
        { label: '本地服务', value: 'local' }
      ]
    }
  },
  {
    key: 'serviceName',
    label: '服务名称',
    type: 'select',
    props: {
      placeholder: '请选择服务名称',
      clearable: true,
      options: Array.from(new Set(tableData.value.map(tool => tool.service_name)))
        .map(svc => ({ label: svc, value: svc }))
    }
  },
  {
    key: 'toolName',
    label: '工具名称',
    type: 'select',
    props: {
      placeholder: '请选择工具名称',
      clearable: true,
      options: availableToolNames.value,
      filterable: true // 支持搜索
    }
  }
])

// 表格数据
const tableData = ref<any[]>([])

// 列显示控制
const columnChecks = ref([
  { key: 'name', label: '工具名称', checked: true },
  { key: 'serviceName', label: '服务名称', checked: true },
  { key: 'type', label: '工具类型', checked: true },
  { key: 'description', label: '描述', checked: true },
  { key: 'status', label: '状态', checked: true },
  { key: 'callCount', label: '调用次数', checked: true },
  { key: 'successRate', label: '成功率', checked: true },
  { key: 'avgResponseTime', label: '平均响应时间', checked: true },
  { key: 'lastUsed', label: '最后使用', checked: true },
  { key: 'actions', label: '操作', checked: true }
])

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
const tableColumns = computed(() => [
  { type: 'selection', width: 55 },
  { prop: 'name', label: '工具名称', minWidth: 180 },
  { prop: 'service_name', label: '服务名称', width: 160 },
  { prop: 'description', label: '描述', minWidth: 260 },
  { prop: 'has_schema', label: '输入模式', width: 100, useSlot: true },
  { prop: 'actions', label: '操作', width: 160, useSlot: true, fixed: 'right' }
].filter(col => {
  const check = columnChecks.value.find(c => c.key === col.prop || c.key === col.type)
  return !check || check.checked
}))

// 计算属性
const totalTools = computed(() => tableData.value.length)
const successRate = computed(() => {
  // 基于工具数据计算成功率，如果没有调用数据则返回100%
  if (tableData.value.length === 0) return 100
  const activeTools = tableData.value.filter(tool => tool.has_schema).length
  return totalTools.value > 0 ? Math.round((activeTools / totalTools.value) * 100) : 100
})

const successChartData = computed(() => [
  { value: successRate.value, name: '成功' },
  { value: 100 - successRate.value, name: '失败' }
])

const toolTypeData = computed(() => {
  const services = tableData.value.reduce((acc, tool) => {
    const svc = tool.service_name || 'unknown'
    acc[svc] = (acc[svc] || 0) + 1
    return acc
  }, {} as Record<string, number>)
  return Object.values(services)
})

// 加载工具：使用 MCP 实际接口 /for_store/list_tools
const loadTools = async () => {
  tableLoading.value = true
  try {
    const res = await dashboardApi.getTools() // { success, data: [...], metadata: {...} }
    const tools = res?.data || []
    tableData.value = tools

    // 服务名称选项会通过计算属性自动更新，这里不需要手动更新

    // 更新分页
    pagination.total = tableData.value.length
  } catch (e) {
    console.error(e)
    ElMessage.error('获取工具列表失败')
  } finally {
    tableLoading.value = false
  }
}


const toolTypeLabels = computed(() => {
  const services = tableData.value.reduce((acc, tool) => {
    const svc = tool.service_name || 'unknown'
    acc[svc] = (acc[svc] || 0) + 1
    return acc
  }, {} as Record<string, number>)
  return Object.keys(services)
})

// 动态的工具名称选项
const availableToolNames = computed(() => {
  let filteredTools = tableData.value
  
  // 根据服务类型筛选
  if (searchForm.value.serviceType) {
    filteredTools = filteredTools.filter(tool => tool.serviceType === searchForm.value.serviceType)
  }
  
  // 根据服务名称筛选
  if (searchForm.value.serviceName) {
    filteredTools = filteredTools.filter(tool => tool.service_name === searchForm.value.serviceName)
  }
  
  // 获取工具名称并去重
  const toolNames = Array.from(new Set(filteredTools.map(tool => tool.name)))
  return toolNames.map(name => ({ label: name, value: name }))
})

// 筛选后的表格数据
const filteredTableData = computed(() => {
  let filtered = tableData.value.filter(tool => {
    const { serviceType, serviceName, toolName } = searchForm.value

    if (serviceType && tool.serviceType !== serviceType) return false
    if (serviceName && tool.serviceName !== serviceName) return false
    if (toolName && !tool.name.toLowerCase().includes(toolName.toLowerCase())) return false

    return true
  })

  // 更新分页总数
  pagination.total = filtered.length

  // 分页处理
  const start = (pagination.currentPage - 1) * pagination.pageSize
  const end = start + pagination.pageSize
  return filtered.slice(start, end)
})

// 方法
const getTypeTagType = (type: string) => {
  const typeMap: Record<string, string> = {
    search: 'primary',
    query: 'success',
    file: 'warning',
    system: 'danger'
  }
  return typeMap[type] || 'info'
}

const getStatusType = (status: string) => {
  return status === 'active' ? 'success' : 'info'
}

const getStatusText = (status: string) => {
  return status === 'active' ? '活跃' : '非活跃'
}

const refreshTools = async () => {
  await loadTools()
  ElMessage.success('工具数据已刷新')
}

const refreshTable = async () => {
  await loadTools()
  ElMessage.success('表格数据已刷新')
}

const handleSearch = () => {
  pagination.currentPage = 1
  ElMessage.info('搜索完成')
}

const handleReset = () => {
  pagination.currentPage = 1
  // 重置时清空工具名称
  searchForm.value.toolName = ''
  ElMessage.info('筛选条件已重置')
}

// 监听服务类型和服务名称变化，自动清空工具名称
watch([() => searchForm.value.serviceType, () => searchForm.value.serviceName], () => {
  // 当服务类型或服务名称改变时，清空工具名称选择
  searchForm.value.toolName = ''
})

const handleSelectionChange = (selection: any[]) => {
  selectedRows.value = selection
}

const handleSizeChange = (size: number) => {
  pagination.pageSize = size
  pagination.currentPage = 1
}

const handleCurrentChange = (page: number) => {
  pagination.currentPage = page
}

const executeTool = (row: any) => {
  // 跳转到工具执行器页面
  router.push({
    path: '/tools/execute',
    query: { 
      toolName: row.name,
      serviceName: row.service_name
    }
  })
}

const viewTool = (row: any) => {
  // 跳转到工具详情页面
  router.push({
    path: '/tools/detail',
    query: { 
      toolName: row.name,
      serviceName: row.service_name
    }
  })
}

const testTool = (row: any) => {
  // 跳转到工具执行器页面进行测试
  router.push({
    path: '/tools/execute',
    query: { 
      toolName: row.name,
      serviceName: row.service_name,
      mode: 'test' // 添加测试模式标识
    }
  })
}

const batchExecute = () => {
  if (selectedRows.value.length === 0) {
    ElMessage.warning('请先选择要执行的工具')
    return
  }
  
  // 跳转到批量执行页面
  const toolNames = selectedRows.value.map(row => row.name)
  router.push({
    path: '/tools/batch-execute',
    query: { 
      tools: toolNames.join(',')
    }
  })
}

const exportTools = () => {
  try {
    // 生成CSV格式的工具列表
    const headers = ['工具名称', '服务名称', '描述', '输入模式']
    const csvContent = [
      headers.join(','),
      ...tableData.value.map(tool => [
        tool.name,
        tool.service_name,
        `"${tool.description || ''}"`,
        tool.has_schema ? '有' : '无'
      ].join(','))
    ].join('\n')
    
    // 创建下载链接
    const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' })
    const link = document.createElement('a')
    link.href = URL.createObjectURL(blob)
    link.download = `工具列表_${new Date().toISOString().split('T')[0]}.csv`
    link.click()
    
    ElMessage.success('工具列表导出成功')
  } catch (error) {
    ElMessage.error(`导出失败: ${error}`)
  }
}

onMounted(() => {
  loadTools()
})
</script>

<style lang="scss" scoped>
.tools-page {
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

  // 增强表格卡片样式
  .table-card-enhanced {
    background: var(--el-bg-color);
    border: 1px solid var(--el-border-color-light);
    border-radius: 12px;
    box-shadow: 0 4px 6px rgba(0, 0, 0, 0.07);
    overflow: hidden;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);

    &:hover {
      box-shadow: 0 8px 20px rgba(0, 0, 0, 0.12);
      transform: translateY(-1px);
      border-color: var(--el-color-primary-light-7);
    }

    // 增强卡片内的表格样式
    :deep(.el-table) {
      background: transparent;
      
      .el-table__header {
        background: var(--el-fill-color-lighter);
        
        th {
          background: transparent;
          border-bottom: 2px solid var(--el-border-color-light);
          font-weight: 600;
          color: var(--el-text-color-primary);
        }
      }

      .el-table__body {
        tr {
          transition: all 0.2s ease;
          
          &:hover {
            background: var(--el-color-primary-light-9);
            transform: scale(1.001);
          }
        }
        
        td {
          border-bottom: 1px solid var(--el-border-color-lighter);
          padding: 12px 0;
        }
      }
    }

    // 增强分页样式
    :deep(.el-pagination) {
      padding: 20px 0 10px;
      background: var(--el-fill-color-extra-light);
      border-radius: 0 0 12px 12px;
      margin: 20px -20px -20px;
      
      .el-pagination__total,
      .el-pager li,
      .el-pagination__jump {
        background: transparent;
      }
      
      .el-pager li.is-active {
        background: var(--el-color-primary);
        border-radius: 6px;
        color: #fff;
        font-weight: 600;
      }
    }
  }

  // 响应式设计
  @media (max-width: 768px) {
    padding: 12px;

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
