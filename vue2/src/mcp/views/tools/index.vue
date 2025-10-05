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

    <!-- 关键词搜索 -->
    <div class="keyword-search mb-3">
      <el-input v-model="keyword" placeholder="Search tools or services" clearable class="kw-input" />
      <el-button class="kw-btn" type="primary" @click="refreshTable">Search</el-button>
      <el-button class="kw-btn" @click="clearKeyword">Clear</el-button>
    </div>

    <!-- 原有筛选条：保留选择框（服务类型/名称/工具名称） -->
    <ArtSearchBar
      v-model="searchForm"
      :items="searchFormItems"
      @search="handleSearch"
      @reset="handleReset"
      class="mb-3"
    />

    <!-- 工具列表表格 -->
    <el-card class="art-table-card table-card-enhanced" shadow="hover">
      <ArtTableHeader :loading="tableLoading" @refresh="refreshTable" />

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
        <!-- Inputs chips 展示 -->
        <template #inputs="{ row }">
          <div class="inputs-wrap">
            <span v-for="it in row.inputsList" :key="it.key" class="input-chip">
              <span class="key" :title="it.key">{{ it.key }}<span v-if="it.required" class="req">*</span></span>
              <span class="type">{{ it.type }}</span>
              <span v-if="it.extras" class="extras">{{ it.extras }}</span>
            </span>
            <span v-if="!row.inputsList || row.inputsList.length === 0" class="input-chip empty">None</span>
          </div>
        </template>
        <!-- 操作插槽（文字链接） -->
        <template #actions="{ row }">
          <div style="display:flex; gap:8px; justify-content:flex-end;">
            <span class="action-link" @click="viewTool(row)">详情</span>
            <span class="action-link" @click="executeTool(row)">执行</span>
          </div>
        </template>
      </ArtTable>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, computed, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { mcpApi } from '../../api/index'
import { useRouter } from 'vue-router'

const router = useRouter()
const headerSubtitle = computed(() => {
  return `当前共有 ${totalTools.value} 个工具，来源于 ${new Set(tableData.value.map(t => t.service)).size} 个服务。`
})


defineOptions({ name: 'ToolList' })

// 响应式数据
const tableLoading = ref(false)
const selectedRows = ref<any[]>([])

// 单关键词搜索
const keyword = ref('')

// 表格数据
const tableData = ref<any[]>([])

const columnChecks = ref([])

// 保留原有筛选表单
const searchForm = ref({
  serviceType: '',
  serviceName: '',
  toolName: ''
})

const searchFormItems = computed(() => [
  {
    key: 'serviceType',
    label: '服务类型',
    type: 'select',
    props: {
      placeholder: '请选择服务类型',
      clearable: true,
      options: [
        { label: 'remote', value: 'remote' },
        { label: 'local', value: 'local' }
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
      options: Array.from(new Set(tableData.value.map(tool => tool.service)))
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
      filterable: true
    }
  }
])

// 分页配置
const pagination = reactive<any>({
  currentPage: 1,
  pageSize: 10,
  total: 0,
  showSizeChanger: true,
  showQuickJumper: true,
  showTotal: true
})

// 表格列配置
const tableColumns = computed(() => ([
  { prop: 'id', label: '#', width: 60 },
  { prop: 'name', label: 'Name', minWidth: 200 },
  { prop: 'service', label: 'Service', width: 160 },
  { prop: 'description', label: 'Description', minWidth: 300 },
  { prop: 'inputs', label: 'Inputs', minWidth: 260, useSlot: true },
  { prop: 'actions', label: 'Actions', headerAlign: 'center', width: 140, useSlot: true, fixed: 'right' }
]) as any)

// 计算属性
const totalTools = computed(() => tableData.value.length)
const successRate = computed(() => {
  if (tableData.value.length === 0) return 100
  const withInputs = tableData.value.filter(tool => (tool.inputs?.length || 0) > 0).length
  return totalTools.value > 0 ? Math.round((withInputs / totalTools.value) * 100) : 100
})

const successChartData = computed(() => [
  { value: successRate.value, name: '成功' },
  { value: 100 - successRate.value, name: '失败' }
])

const toolTypeData = computed(() => {
  const services = tableData.value.reduce((acc, tool) => {
    const svc = tool.service || 'unknown'
    acc[svc] = (acc[svc] || 0) + 1
    return acc
  }, {} as Record<string, number>)
  return Object.values(services) as number[]
})

// 加载工具：使用 MCP 实际接口 /for_store/list_tools
const serviceTypeMap = ref<Record<string, string>>({})

const loadTools = async () => {
  tableLoading.value = true
  try {
    // 获取服务类型映射
    const services = await mcpApi.listServices()
    const map: Record<string, string> = {}
    ;(services || []).forEach((s: any) => {
      const confType = s?.config?.type
      const derived = confType || (s.type === 'streamable_http' || s.type === 'url' ? 'remote' : 'local')
      map[s.name] = derived
    })
    serviceTypeMap.value = map

    // 获取工具
    const tools = await mcpApi.listTools()
    tableData.value = (tools || []).map((t: any, idx: number) => {
      const inputsList = schemaToList(t.input_schema)
      return {
        id: idx + 1,
        name: t.name,
        service: t.service || '',
        serviceType: map[t.service || ''] || '',
        description: t.description || '',
        inputs: summarizeInputs(t.input_schema),
        inputsList
      }
    })

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
    const svc = tool.service || 'unknown'
    acc[svc] = (acc[svc] || 0) + 1
    return acc
  }, {} as Record<string, number>)
  return Object.keys(services)
})

// 关键词过滤
const availableToolNames = computed(() => {
  let list = tableData.value
  if (searchForm.value.serviceType) {
    list = list.filter(t => t.serviceType === searchForm.value.serviceType)
  }
  if (searchForm.value.serviceName) {
    list = list.filter(t => t.service === searchForm.value.serviceName)
  }
  const names = Array.from(new Set(list.map(t => t.name)))
  return names.map(n => ({ label: n, value: n }))
})

// 筛选后的表格数据
const filteredTableData = computed(() => {
  const kw = (keyword.value || '').trim().toLowerCase()
  let filtered = tableData.value.filter(tool => {
    if (searchForm.value.serviceType && tool.serviceType !== searchForm.value.serviceType) return false
    if (searchForm.value.serviceName && tool.service !== searchForm.value.serviceName) return false
    if (searchForm.value.toolName && tool.name !== searchForm.value.toolName) return false
    if (!kw) return true
    const haystack = [tool.name, tool.service, tool.description, tool.inputs]
      .filter(Boolean)
      .join(' ')
      .toLowerCase()
    return haystack.includes(kw)
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

const clearKeyword = () => {
  keyword.value = ''
  pagination.currentPage = 1
}

const handleSearch = () => {
  pagination.currentPage = 1
}

const handleReset = () => {
  searchForm.value.serviceType = ''
  searchForm.value.serviceName = ''
  searchForm.value.toolName = ''
  pagination.currentPage = 1
}

watch([() => searchForm.value.serviceType, () => searchForm.value.serviceName], () => {
  searchForm.value.toolName = ''
})

// 无额外筛选监听

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
      serviceName: row.service
    }
  })
}

const viewTool = (row: any) => {
  // 跳转到工具详情页面
  router.push({
    path: '/tools/detail',
    query: { 
      toolName: row.name,
      serviceName: row.service
    }
  })
}

const testTool = (row: any) => {
  // 跳转到工具执行器页面进行测试
  router.push({
    path: '/tools/execute',
    query: { 
      toolName: row.name,
      serviceName: row.service,
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
        tool.service,
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

function summarizeInputs(schema: any): string {
  if (!schema || typeof schema !== 'object') return ''
  if (schema.type !== 'object' || !schema.properties) return ''
  const required: string[] = Array.isArray(schema.required) ? schema.required : []
  const props = schema.properties || {}
  const parts: string[] = []
  for (const key of Object.keys(props)) {
    const p: any = props[key] || {}
    const isReq = required.includes(key)
    const type = p.type || 'any'
    const extras: string[] = []
    if (p.default !== undefined) extras.push(`default:${String(p.default)}`)
    if (typeof p.minimum === 'number') extras.push(`min:${p.minimum}`)
    if (typeof p.maximum === 'number') extras.push(`max:${p.maximum}`)
    const extraText = extras.length ? ` (${extras.join(', ')})` : ''
    parts.push(`${key}${isReq ? '*' : ''}: ${type}${extraText}`)
  }
  return parts.join(', ')
}

function schemaToList(schema: any): Array<{ key: string; required: boolean; type: string; extras: string }> {
  const list: Array<{ key: string; required: boolean; type: string; extras: string }> = []
  if (!schema || schema.type !== 'object' || !schema.properties) return list
  const required: string[] = Array.isArray(schema.required) ? schema.required : []
  const props = schema.properties || {}
  for (const key of Object.keys(props)) {
    const p: any = props[key] || {}
    const type = p.type || 'any'
    const extras: string[] = []
    if (p.default !== undefined) extras.push(`default:${String(p.default)}`)
    if (typeof p.minimum === 'number') extras.push(`min:${p.minimum}`)
    if (typeof p.maximum === 'number') extras.push(`max:${p.maximum}`)
    list.push({ key, required: required.includes(key), type, extras: extras.join(', ') })
  }
  return list
}
</script>

<style lang="scss" scoped>
.tools-page {
  .keyword-search {
    display: flex;
    align-items: center;
    gap: 8px;
    .kw-input { max-width: 360px; }
  }

  .inputs-wrap { display: flex; gap: 6px; flex-wrap: wrap; }
  .input-chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    border-radius: 6px;
    background: var(--el-fill-color-light);
    border: 1px solid var(--el-border-color-lighter);
    color: var(--el-text-color-primary);
    font-size: 12px;
  }
  .input-chip .key { font-weight: 600; }
  .input-chip .req { color: var(--el-color-danger); margin-left: 2px; }
  .input-chip .type { color: var(--el-text-color-secondary); }
  .input-chip .extras { color: var(--el-text-color-secondary); }
  .input-chip.empty { opacity: 0.7; font-style: italic; }
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
.action-link {
  color: var(--el-color-primary);
  cursor: pointer;
  user-select: none;
}
.action-link:hover {
  text-decoration: underline;
}
</style>
