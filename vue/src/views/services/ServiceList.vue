<template>
  <div class="service-list page-container page-container--wide">
    <!-- 正常内容 -->
    <div v-loading="pageLoading" element-loading-text="加载服务数据..." class="service-list__content content-stack">
      <!-- 页面头部 -->
      <div class="page-header">
        <div class="page-header__title-group">
          <h2 class="page-header__title">服务列表</h2>
          <p class="page-header__subtitle">管理所有已注册的MCP服务</p>
        </div>
        <div class="page-header__actions">
          <el-button
            :icon="Plus"
            @click="$router.push('/for_store/add_service')"
          >
            添加服务
          </el-button>
          <el-button
            :icon="Refresh"
            @click="refreshServices"
            :loading="refreshLoading"
          >
            刷新
          </el-button>
          <el-dropdown @command="handleQuickAction">
            <el-button :icon="Tools">
              快速操作
              <el-icon class="el-icon--right"><ArrowDown /></el-icon>
            </el-button>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item command="reset-config" :icon="RefreshLeft">
                  重置Store配置
                </el-dropdown-item>
                <el-dropdown-item command="reset-manager" :icon="Setting">
                  重置管理
                </el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </div>
      </div>

      <!-- 左右布局：左侧筛选+表格，右侧图表 -->
      <el-row :gutter="16" class="list-layout">
        <el-col :span="16" class="list-col">
          <el-card class="list-card">
            <template #header>
              <div class="list-toolbar">
                <el-input
                  v-model="searchQuery"
                  placeholder="Search by name"
                  :prefix-icon="Search"
                  clearable
                  @input="handleSearch"
                  class="toolbar-input"
                />
                <el-select
                  v-model="typeFilter"
                  placeholder="Type"
                  clearable
                  @change="handleFilter"
                  class="toolbar-select"
                >
                  <el-option label="All Types" value="" />
                  <el-option label="streamable_http" value="streamable_http" />
                  <el-option label="stdio" value="stdio" />
                  <el-option label="sse" value="sse" />
                </el-select>
                <el-select
                  v-model="statusFilter"
                  placeholder="Status"
                  clearable
                  @change="handleFilter"
                  class="toolbar-select"
                >
                  <el-option label="All Status" value="" />
                  <el-option label="healthy" value="healthy" />
                  <el-option label="initializing" value="initializing" />
                  <el-option label="warning" value="warning" />
                  <el-option label="reconnecting" value="reconnecting" />
                  <el-option label="unreachable" value="unreachable" />
                  <el-option label="disconnecting" value="disconnecting" />
                  <el-option label="disconnected" value="disconnected" />
                </el-select>
                <div class="toolbar-spacer" />
                <el-button :icon="Refresh" @click="refreshServices" :loading="refreshLoading">刷新</el-button>
                <el-dropdown @command="handleBatchAction" :disabled="selectedServices.length === 0">
                  <el-button :icon="Operation" :disabled="selectedServices.length === 0">批量</el-button>
                  <template #dropdown>
                    <el-dropdown-menu>
                      <el-dropdown-item command="batch-restart" :icon="Refresh">
                        批量重启
                      </el-dropdown-item>
                      <el-dropdown-item command="batch-delete" :icon="Delete" divided>
                        批量删除
                      </el-dropdown-item>
                    </el-dropdown-menu>
                  </template>
                </el-dropdown>
              </div>
            </template>

            <!-- Batch Operations Component -->
            <BatchOperations
              ref="batchOperationsRef"
              :items="systemStore.services"
              item-key="name"
              item-name="name"
              :show-header="false"
              :show-overlay="true"
              :editable-fields="[
                { label: '超时时间', value: 'timeout', type: 'number' },
                { label: '传输协议', value: 'transport', type: 'select', options: [
                  { label: 'HTTP', value: 'http' },
                  { label: 'WebSocket', value: 'ws' },
                  { label: 'Streamable HTTP', value: 'streamable-http' }
                ] }
              ]"
              @batch-edit="handleBatchEdit"
              @batch-delete="handleBatchDelete"
              @selection-change="handleBatchSelectionChange"
            />

            <!-- 服务表格 -->
            <div class="table-wrap">
              <el-table
                v-loading="loading"
                :data="filteredServices"
                @selection-change="handleSelectionChange"
                stripe
                style="width: 100%"
                height="100%"
              >
            <el-table-column type="selection" width="50" />
            <el-table-column type="index" label="#" width="60" />
            <el-table-column prop="name" label="Name" min-width="180" />
            <el-table-column prop="type" label="Type" width="150" />
            <el-table-column label="Status" width="120">
              <template #default="{ row }">
                <el-tag :type="getStatusType(row.status)" size="small">
                  {{ row.status }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column label="KeepAlive" width="110" align="center">
              <template #default="{ row }">
                <el-tag :type="row.keep_alive ? 'success' : 'info'" size="small">
                  {{ row.keep_alive ? 'Yes' : 'No' }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="url" label="URL" min-width="220" show-overflow-tooltip />
            <el-table-column prop="command" label="Command" width="120" />
            <el-table-column label="Args" min-width="200" show-overflow-tooltip>
              <template #default="{ row }">
                <span v-if="row.args && row.args.length">{{ (row.args || []).join(' ') }}</span>
                <span v-else class="text-muted">-</span>
              </template>
            </el-table-column>
            <el-table-column prop="tools_count" label="Tools" width="100" align="center">
              <template #default="{ row }">
                <el-badge :value="row.tools_count ?? 0" :max="99" type="primary" />
              </template>
            </el-table-column>
            <el-table-column label="Actions" width="220" fixed="right" align="center">
              <template #default="{ row }">
                <el-button link size="small" @click="viewServiceDetails(row)">Detail</el-button>
                <el-button link size="small" @click="restartService(row)" :loading="row.restarting">Restart</el-button>
                <el-button link size="small" @click="deleteService(row)">Delete</el-button>
              </template>
            </el-table-column>
              </el-table>
            </div>
          </el-card>
        </el-col>
        <el-col :span="8" class="chart-col">
          <el-card class="chart-card">
            <template #header>
              <span>工具数量分布（按服务）</span>
            </template>
            <div class="chart-content"><div ref="serviceChartRef" class="chart-canvas" /></div>
          </el-card>
        </el-col>
      </el-row>

      <!-- Batch Operations Component -->
      <div class="page-section">
        <BatchOperations
          ref="batchOperationsRef"
          :items="systemStore.services"
          item-key="name"
          item-name="name"
          :show-header="false"
          :show-overlay="true"
          :editable-fields="[
            { label: '超时时间', value: 'timeout', type: 'number' },
            { label: '传输协议', value: 'transport', type: 'select', options: [
              { label: 'HTTP', value: 'http' },
              { label: 'WebSocket', value: 'ws' },
              { label: 'Streamable HTTP', value: 'streamable-http' }
            ] }
          ]"
          @batch-edit="handleBatchEdit"
          @batch-delete="handleBatchDelete"
          @selection-change="handleBatchSelectionChange"
        />

        <!-- 服务表格 -->
        
      </div>

    <!-- 批量更新对话框 -->
    <BatchUpdateDialog
      v-model="batchUpdateDialogVisible"
      :services="selectedServices"
      @updated="handleBatchUpdateSuccess"
    />

    <!-- 编辑服务弹窗 -->
    <el-dialog
      v-model="editDialogVisible"
      :title="`编辑服务 - ${editingService?.name}`"
      width="800px"
      :close-on-click-modal="false"
    >
      <div v-if="editingService" class="edit-service-content">
        <!-- 编辑模式选择 -->
        <div class="edit-mode-selector">
          <el-radio-group v-model="editMode" size="large">
            <el-radio-button label="fields">字段编辑</el-radio-button>
            <el-radio-button label="json">JSON编辑</el-radio-button>
          </el-radio-group>
        </div>

        <!-- 字段编辑模式 -->
        <div v-if="editMode === 'fields'" class="fields-edit-mode">
          <!-- Client ID 展示 -->
          <div v-if="editingServiceClientId" class="client-id-display">
            <el-form-item label="client_id">
              <el-input
                :value="editingServiceClientId"
                readonly
                size="large"
                class="readonly-field"
              >
                <template #suffix>
                  <el-icon class="readonly-icon"><View /></el-icon>
                </template>
              </el-input>
            </el-form-item>
          </div>

          <el-form
            ref="editFormRef"
            :model="editForm"
            label-position="top"
            class="edit-form"
          >
            <!-- 远程服务字段 -->
            <template v-if="isRemoteService">
              <el-form-item label="url" prop="url">
                <el-input
                  v-model="editForm.url"
                  placeholder="Enter service URL, e.g.: https://example.com/mcp"
                  size="large"
                />
              </el-form-item>

              <el-form-item label="transport" prop="transport" v-if="editForm.transport !== undefined">
                <el-input
                  v-model="editForm.transport"
                  placeholder="Enter transport type, e.g.: streamable-http"
                  size="large"
                />
              </el-form-item>

              <el-form-item label="timeout" prop="timeout" v-if="editForm.timeout !== undefined">
                <el-input-number
                  v-model="editForm.timeout"
                  :min="1"
                  :max="300"
                  placeholder="Timeout in seconds"
                  size="large"
                  style="width: 100%"
                />
              </el-form-item>
            </template>

            <!-- 本地服务字段 -->
            <template v-else>
              <el-form-item label="command" prop="command">
                <el-input
                  v-model="editForm.command"
                  placeholder="Enter command, e.g.: npx, python"
                  size="large"
                />
              </el-form-item>

              <el-form-item label="args" prop="args" v-if="editForm.args !== undefined">
                <el-input
                  v-model="editFormArgsString"
                  placeholder="Enter arguments separated by spaces"
                  size="large"
                />
                <div class="field-hint">Arguments will be split by spaces into an array</div>
              </el-form-item>

              <el-form-item label="working_dir" prop="working_dir" v-if="editForm.working_dir !== undefined">
                <el-input
                  v-model="editForm.working_dir"
                  placeholder="Enter working directory path (optional)"
                  size="large"
                />
              </el-form-item>
            </template>

            <!-- 通用字段 -->
            <el-form-item label="env" prop="env" v-if="editForm.env !== undefined">
              <el-input
                v-model="editFormEnvString"
                type="textarea"
                :rows="3"
                placeholder="Enter environment variables, format: KEY1=value1&#10;KEY2=value2"
                size="large"
              />
              <div class="field-hint">One environment variable per line, format: KEY=value</div>
            </el-form-item>
          </el-form>
        </div>

        <!-- JSON编辑模式 -->
        <div v-else class="json-edit-mode">
          <el-form-item label="配置内容">
            <el-input
              v-model="editJsonContent"
              type="textarea"
              :rows="12"
              placeholder="请输入JSON配置内容"
              size="large"
            />
          </el-form-item>

          <div class="json-actions">
            <el-button @click="formatEditJson" size="large">
              <el-icon><Setting /></el-icon>
              格式化
            </el-button>
            <el-button @click="validateEditJson" size="large">
              <el-icon><Check /></el-icon>
              验证
            </el-button>
          </div>
        </div>
      </div>

      <template #footer>
        <div class="dialog-footer">
          <el-button @click="editDialogVisible = false">取消</el-button>
          <el-button
            type="primary"
            @click="saveServiceEdit"
            :loading="editSaving"
          >
            保存
          </el-button>
        </div>
      </template>
    </el-dialog>

    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, nextTick, onUnmounted, watch } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import { ElMessage, ElMessageBox } from 'element-plus'
import dayjs from 'dayjs'
import * as echarts from 'echarts'
import BatchOperations from '@/components/BatchOperations.vue'
import BatchUpdateDialog from './BatchUpdateDialog.vue'
import { SERVICE_STATUS_COLORS, SERVICE_STATUS_MAP } from '@/utils/constants'
import {
  Plus, Refresh, Search, Delete, Connection, FolderOpened,
  Link, Tools, View, ArrowDown, RefreshLeft, Setting, Operation, Edit,
  Warning, Check
} from '@element-plus/icons-vue'

const router = useRouter()
const route = useRoute()
const systemStore = useSystemStore()

// 响应式数据
const loading = ref(false)
const pageLoading = ref(false)
const refreshLoading = ref(false)
const searchQuery = ref('')
const statusFilter = ref('')
const typeFilter = ref('')
const batchOperationsRef = ref(null)
const selectedServices = ref([])
const batchUpdateDialogVisible = ref(false)
const servicesData = ref(null) // 存储API返回的完整数据

// 编辑服务相关数据
const editDialogVisible = ref(false)
const editingService = ref(null)
const editingServiceClientId = ref('')
const editMode = ref('fields') // 'fields' | 'json'
const editForm = ref({})
const editJsonContent = ref('')
const editSaving = ref(false)
const editFormRef = ref()
const editFormArgsString = ref('')
const editFormEnvString = ref('')

// 计算属性：判断是否为远程服务
const isRemoteService = computed(() => {
  return editForm.value.url && !editForm.value.command
})

// 计算属性
const filteredServices = computed(() => {
  let services = systemStore.services

  // 搜索过滤
  if (searchQuery.value) {
    const query = searchQuery.value.toLowerCase()
    services = services.filter(service =>
      service.name.toLowerCase().includes(query) ||
      (service.url && service.url.toLowerCase().includes(query)) ||
      (service.command && service.command.toLowerCase().includes(query))
    )
  }

  // 🔧 改进：状态过滤支持激活状态和7状态系统
  if (statusFilter.value) {
    services = services.filter(service => {
      if (statusFilter.value === 'active') {
        return service.is_active === true
      } else if (statusFilter.value === 'config-only') {
        return service.is_active === false
      } else {
        // 具体状态过滤
        return service.status === statusFilter.value
      }
    })
  }

  // 类型过滤（直接匹配 type 字段）
  if (typeFilter.value) {
    services = services.filter(service => service.type === typeFilter.value)
  }

  return services
})

// 图表：工具数量分布（按服务）
const serviceChartRef = ref(null)
let serviceChart
let resizeObserver
let chartInitAttempts = 0

const renderServiceChart = () => {
  if (!serviceChart) {
    console.warn('📊 [图表调试] serviceChart 未初始化')
    return
  }
  const serviceNames = systemStore.services.map(s => s.name)
  const counts = systemStore.services.map(s => Number(s.tools_count || 0))
  
  console.log('📊 [图表调试] 渲染图表数据:', { 
    服务数量: serviceNames.length, 
    服务名: serviceNames, 
    工具数: counts 
  })
  
  if (serviceNames.length === 0) {
    console.warn('📊 [图表调试] 没有服务数据，显示空状态')
    // 显示空状态
    serviceChart.setOption({
      title: {
        text: '暂无服务数据',
        left: 'center',
        top: 'center',
        textStyle: {
          color: '#999',
          fontSize: 14
        }
      }
    })
    return
  }
  
  serviceChart.setOption({
    grid: { left: 40, right: 10, top: 20, bottom: 40 },
    xAxis: { type: 'category', data: serviceNames, axisLabel: { rotate: 30, interval: 0 } },
    yAxis: { type: 'value' },
    tooltip: { trigger: 'axis' },
    series: [{ type: 'bar', data: counts, itemStyle: { color: '#409EFF' } }]
  })
  
  console.log('✅ [图表调试] 图表渲染完成')
}

const tryInitChart = () => {
  const el = serviceChartRef.value
  if (!el) {
    console.warn('📊 [图表调试] 图表容器DOM未找到')
    return
  }
  
  const { clientWidth, clientHeight } = el
  console.log('📊 [图表调试] 图表容器尺寸:', { width: clientWidth, height: clientHeight, attempts: chartInitAttempts })
  
  if (clientWidth && clientHeight) {
    if (!serviceChart) {
      console.log('📊 [图表调试] 初始化ECharts实例...')
      serviceChart = echarts.init(el)
      console.log('✅ [图表调试] ECharts实例初始化完成')
    }
    renderServiceChart()
  } else {
    // 容器尚未完成布局，延迟重试
    if (chartInitAttempts < 30) {
      chartInitAttempts += 1
      console.log(`⏳ [图表调试] 容器尺寸为0，延迟重试 (${chartInitAttempts}/30)`)
      setTimeout(tryInitChart, 100)
    } else {
      console.error('❌ [图表调试] 图表初始化失败：容器尺寸始终为0，已达最大重试次数')
    }
  }
}

const resizeChart = () => {
  if (serviceChart) serviceChart.resize()
}

// 列表高度由卡片body填充，无需固定值

// 健康服务数量计算
const healthyServicesCount = computed(() => {
  return systemStore.services.filter(service => service.status === 'healthy').length
})

// 🔧 改进：状态处理函数支持7状态系统
const getStatusType = (status) => {
  switch (status) {
    case 'initializing': return 'primary'
    case 'healthy': return 'success'
    case 'warning': return 'warning'
    case 'reconnecting': return 'primary'
    case 'unreachable': return 'danger'
    case 'disconnecting': return 'warning'
    case 'disconnected': return 'info'
    default: return 'info'
  }
}

const getStatusText = (status) => {
  return SERVICE_STATUS_MAP[status] || '未知'
}

// 方法
const refreshServices = async () => {
  refreshLoading.value = true
  try {
    const { api } = await import('@/api')
    const servicesArr = await api.store.listServices()
    servicesData.value = { services: servicesArr, total_services: servicesArr.length }
    await systemStore.fetchServices(true)

    ElMessage.success('服务列表刷新成功')
  } catch (error) {
    console.error('刷新服务列表失败:', error)
    ElMessage.error('刷新失败')
  } finally {
    refreshLoading.value = false
  }
}

const handleSearch = () => {
  // 搜索逻辑已在计算属性中处理
}

const handleFilter = () => {
  // 过滤逻辑已在计算属性中处理
}

const handleSelectionChange = (selection) => {
  selectedServices.value = selection
  // Sync with batch operations component
  if (batchOperationsRef.value) {
    batchOperationsRef.value.selectedItems = selection
  }
}

const handleBatchSelectionChange = (selection) => {
  selectedServices.value = selection
}

const handleBatchAction = async (command) => {
  if (selectedServices.value.length === 0) return

  switch (command) {
    case 'batch-update':
      if (batchOperationsRef.value) {
        batchOperationsRef.value.showBatchEditDialog()
      }
      break
    case 'batch-restart':
      await handleBatchRestart()
      break
    case 'batch-delete':
      if (batchOperationsRef.value) {
        batchOperationsRef.value.showBatchDeleteDialog()
      }
      break
  }
}

const handleBatchUpdate = async () => {
  if (selectedServices.value.length === 0) {
    ElMessage.warning('请先选择要更新的服务')
    return
  }
  batchUpdateDialogVisible.value = true
}

const handleBatchUpdateSuccess = async () => {
  await refreshServices()
  selectedServices.value = []
}

const handleBatchRestart = async () => {
  try {
    await ElMessageBox.confirm(
      `确定要重启选中的 ${selectedServices.value.length} 个服务吗？`,
      '批量重启确认',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )

    const { api } = await import('@/api')
    const serviceNames = selectedServices.value.map(s => s.name)
    const response = await api.store.batchRestartServices(serviceNames)

    if (response.data.success) {
      ElMessage.success('批量重启成功')
      await refreshServices()
    } else {
      ElMessage.error(response.data.message || '批量重启失败')
    }

    selectedServices.value = []
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error('批量重启失败')
    }
  }
}

const handleBatchEdit = async (data) => {
  try {
    const { items, field, value } = data
    const serviceNames = items.map(s => s.name)

    // Prepare update data
    const updateData = {}
    updateData[field] = value

    const { api } = await import('@/api')
    const response = await api.store.batchUpdateServices(serviceNames, updateData)

    if (response.data.success) {
      ElMessage.success('批量更新成功')
      await refreshServices()
    } else {
      ElMessage.error(response.data.message || '批量更新失败')
    }
  } catch (error) {
    console.error('Batch edit failed:', error)
    ElMessage.error('批量更新失败')
  }
}

const handleBatchDelete = async (services) => {
  try {
    await ElMessageBox.confirm(
      `确定要删除选中的 ${services.length} 个服务吗？`,
      '批量删除确认',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )

    const { api } = await import('@/api')
    const serviceNames = services.map(s => s.name)
    const response = await api.store.batchDeleteServices(serviceNames)

    if (response.data.success) {
      ElMessage.success('批量删除成功')
      await refreshServices()
    } else {
      ElMessage.error(response.data.message || '批量删除失败')
    }
  } catch (error) {
    if (error !== 'cancel') {
      console.error('Batch delete failed:', error)
      ElMessage.error('批量删除失败')
    }
  }
}

const viewServiceTools = (service) => {
  // 跳转到工具列表页面，并筛选该服务的工具
  router.push({
    path: '/for_store/list_tools',
    query: { service: service.name }
  })
}

const viewServiceDetails = (service) => {
  router.push({
    path: `/for_store/service_info/${service.name}`,
    query: route.query.agent ? { agent: route.query.agent } : {}
  })
}

const restartService = async (service) => {
  try {
    service.restarting = true
    await systemStore.restartService(service.name)
    ElMessage.success(`服务 ${service.name} 重启成功`)
  } catch (error) {
    ElMessage.error(`服务 ${service.name} 重启失败`)
  } finally {
    service.restarting = false
  }
}

const deleteService = async (service) => {
  try {
    await ElMessageBox.confirm(
      `确定要删除服务 "${service.name}" 吗？`,
      '删除确认',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )

    const { api } = await import('@/api')

    // 根据是否有agent参数决定使用哪个API
    const agentId = route.query.agent
    let response

    if (agentId) {
      // Agent级别删除
      response = await api.agent.deleteConfig(agentId, service.name)
    } else {
      // Store级别删除
      response = await api.store.deleteService(service.name)
    }

    if (response.data.success) {
      ElMessage.success(`服务 ${service.name} 删除成功`)
      await refreshServices()
    } else {
      ElMessage.error(response.data.message || `服务 ${service.name} 删除失败`)
    }
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error(`服务 ${service.name} 删除失败: ${error.message}`)
    }
  }
}

const editService = async (service) => {
  try {
    editingService.value = service
    editMode.value = 'fields'

    // 获取服务配置
    const { api } = await import('@/api')
    const agentId = route.query.agent
    let response

    if (agentId) {
      // Agent级别获取配置
      response = await api.agent.showConfig(agentId)
    } else {
      // Store级别获取配置
      response = await api.store.getConfig('global')
    }

    if (response.data.success) {
      // 从配置中找到当前服务的配置和client_id
      let serviceConfig = null
      let clientId = ''

      console.log('🔍 [DEBUG] API响应数据:', response.data.data)

      if (agentId && response.data.data.services) {
        // Agent级别的配置
        const serviceInfo = response.data.data.services[service.name]
        serviceConfig = serviceInfo?.config
        clientId = serviceInfo?.client_id || ''
        console.log('🔍 [DEBUG] Agent级别配置:', serviceConfig, 'Client ID:', clientId)
      } else if (response.data.data.services) {
        // Store级别的配置（直接在services中）
        const serviceInfo = response.data.data.services[service.name]
        serviceConfig = serviceInfo?.config
        clientId = serviceInfo?.client_id || ''
        console.log('🔍 [DEBUG] Store级别配置:', serviceConfig, 'Client ID:', clientId)
      } else if (response.data.data.agents?.global_agent_store?.services) {
        // 嵌套在agents中的配置
        const serviceInfo = response.data.data.agents.global_agent_store.services[service.name]
        serviceConfig = serviceInfo?.config
        clientId = serviceInfo?.client_id || ''
        console.log('🔍 [DEBUG] 嵌套配置:', serviceConfig, 'Client ID:', clientId)
      }

      // 设置client_id
      editingServiceClientId.value = clientId

      if (serviceConfig) {
        // 初始化编辑表单
        editForm.value = { ...serviceConfig }

        // 初始化args字符串字段
        if (serviceConfig.args && Array.isArray(serviceConfig.args)) {
          editFormArgsString.value = serviceConfig.args.join(' ')
        } else {
          editFormArgsString.value = ''
        }

        // 初始化env字符串字段
        if (serviceConfig.env && typeof serviceConfig.env === 'object') {
          editFormEnvString.value = Object.entries(serviceConfig.env)
            .map(([key, value]) => `${key}=${value}`)
            .join('\n')
        } else {
          editFormEnvString.value = ''
        }

        editJsonContent.value = JSON.stringify({ [service.name]: serviceConfig }, null, 2)

        console.log('🔍 [DEBUG] 服务配置加载:', {
          serviceName: service.name,
          serviceConfig,
          editForm: editForm.value,
          argsString: editFormArgsString.value,
          envString: editFormEnvString.value
        })
      } else {
        // 如果没有找到配置，根据服务类型使用默认配置
        if (service.url) {
          // 远程服务
          editForm.value = {
            url: service.url || '',
            transport: service.transport || 'streamable-http',
            timeout: service.timeout || 30
          }
        } else {
          // 本地服务
          editForm.value = {
            command: service.command || '',
            args: service.args || [],
            working_dir: service.working_dir || '',
            env: service.env || {}
          }

          if (Array.isArray(service.args)) {
            editFormArgsString.value = service.args.join(' ')
          }
        }

        // 初始化环境变量字符串
        if (service.env && typeof service.env === 'object') {
          editFormEnvString.value = Object.entries(service.env)
            .map(([key, value]) => `${key}=${value}`)
            .join('\n')
        } else {
          editFormEnvString.value = ''
        }

        editJsonContent.value = JSON.stringify({ [service.name]: editForm.value }, null, 2)
      }

      editDialogVisible.value = true
    } else {
      ElMessage.error('获取服务配置失败')
    }
  } catch (error) {
    ElMessage.error(`获取服务配置失败: ${error.message}`)
  }
}

const formatTime = (time) => {
  return dayjs(time).format('YYYY-MM-DD HH:mm:ss')
}

const formatRelativeTime = (time) => {
  const now = dayjs()
  const target = dayjs(time)
  const diffMinutes = now.diff(target, 'minute')

  if (diffMinutes < 1) {
    return '刚刚'
  } else if (diffMinutes < 60) {
    return `${diffMinutes}分钟前`
  } else if (diffMinutes < 1440) {
    const hours = Math.floor(diffMinutes / 60)
    return `${hours}小时前`
  } else {
    const days = Math.floor(diffMinutes / 1440)
    return `${days}天前`
  }
}

// 🔧 新增：服务激活功能
const activateService = async (service) => {
  try {
    service.activating = true

    const { api } = await import('@/api')
    const response = await api.store.initService(service.name)

    if (response.data.success) {
      ElMessage.success(`服务 ${service.name} 激活成功`)
      await refreshServices()
    } else {
      ElMessage.error(response.data.message || `服务 ${service.name} 激活失败`)
    }
  } catch (error) {
    console.error('激活服务失败:', error)
    ElMessage.error(`服务 ${service.name} 激活失败`)
  } finally {
    service.activating = false
  }
}

// 快速操作处理
const handleQuickAction = async (command) => {
  switch (command) {
    case 'reset-config':
      await handleResetStoreConfig()
      break
    case 'reset-manager':
      router.push('/system/reset')
      break
  }
}

const handleResetStoreConfig = async () => {
  try {
    await ElMessageBox.confirm(
      '此操作将重置Store的所有配置，包括内存数据和文件中的相关配置。是否继续？',
      '确认重置',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )

    const { api } = await import('@/api')
    const response = await api.store.resetConfig()

    if (response.data.success) {
      ElMessage.success('Store配置重置成功')
      await refreshServices()
    } else {
      ElMessage.error(response.data.message || 'Store配置重置失败')
    }
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error('Store配置重置失败')
    }
  }
}

// 错误处理函数 - 简化为消息提示
const handleError = (error) => {
  let errorMessage = '加载失败'
  
  if (error.code === 'ECONNREFUSED' || error.code === 'ERR_NETWORK') {
    errorMessage = '无法连接到后端服务，请检查服务是否正常运行'
  } else if (error.response?.status >= 500) {
    errorMessage = '服务器内部错误，请稍后重试'
  } else if (error.code === 'ECONNABORTED' || error.message?.includes('timeout')) {
    errorMessage = '请求超时，请检查网络状况'
  } else if (error.message) {
    errorMessage = error.message
  }
  
  ElMessage.error(errorMessage)
  console.error('服务列表加载失败:', error)
}

// 编辑服务相关方法
const formatEditJson = () => {
  try {
    const parsed = JSON.parse(editJsonContent.value)
    editJsonContent.value = JSON.stringify(parsed, null, 2)
    ElMessage.success('JSON格式化成功')
  } catch (error) {
    ElMessage.error('JSON格式错误')
  }
}

const validateEditJson = () => {
  try {
    JSON.parse(editJsonContent.value)
    ElMessage.success('JSON格式正确')
  } catch (error) {
    ElMessage.error('JSON格式错误: ' + error.message)
  }
}

const saveServiceEdit = async () => {
  if (!editingService.value) return

  try {
    editSaving.value = true

    const { api } = await import('@/api')
    const agentId = route.query.agent
    let config

    if (editMode.value === 'fields') {
      // 字段编辑模式 - 处理不同类型的服务
      config = { ...editForm.value }

      // 处理args字段（从字符串转换为数组）
      if (editFormArgsString.value.trim()) {
        config.args = editFormArgsString.value.trim().split(/\s+/)
      } else if (config.args !== undefined) {
        config.args = []
      }

      // 处理env字段（从字符串转换为对象）
      if (editFormEnvString.value.trim()) {
        config.env = {}
        editFormEnvString.value.split('\n').forEach(line => {
          const trimmedLine = line.trim()
          if (trimmedLine && trimmedLine.includes('=')) {
            const [key, ...valueParts] = trimmedLine.split('=')
            const value = valueParts.join('=')
            if (key.trim()) {
              config.env[key.trim()] = value
            }
          }
        })
      } else if (config.env !== undefined) {
        config.env = {}
      }

      // 清理不相关的字段
      if (isRemoteService.value) {
        // 远程服务：删除本地服务字段
        delete config.command
        delete config.args
        delete config.working_dir
      } else {
        // 本地服务：删除远程服务字段
        delete config.url
        delete config.transport
      }
    } else {
      // JSON编辑模式
      try {
        const parsed = JSON.parse(editJsonContent.value)
        // 提取服务配置
        const serviceName = editingService.value.name
        config = parsed[serviceName] || parsed
      } catch (error) {
        ElMessage.error('JSON格式错误')
        return
      }
    }

    let response
    if (agentId) {
      // Agent级别更新
      response = await api.agent.updateConfig(agentId, editingService.value.name, config)
    } else {
      // Store级别更新
      response = await api.store.updateConfig(editingService.value.name, config)
    }

    if (response.data.success) {
      ElMessage.success('服务配置更新成功')
      editDialogVisible.value = false
      await refreshServices()
    } else {
      ElMessage.error(response.data.message || '服务配置更新失败')
    }
  } catch (error) {
    ElMessage.error(`服务配置更新失败: ${error.message}`)
  } finally {
    editSaving.value = false
  }
}

// 生命周期
onMounted(async () => {
  console.log('🚀 [服务列表] 组件挂载开始')
  pageLoading.value = true
  try {
    console.log('📡 [服务列表] 开始加载服务数据...')
    const { api } = await import('@/api')
    const servicesArr = await api.store.listServices()
    console.log('✅ [服务列表] 服务数据加载完成:', servicesArr.length, '个服务')
    servicesData.value = { services: servicesArr, total_services: servicesArr.length }
    await systemStore.fetchServices(true)
    
    console.log('🎨 [服务列表] 等待DOM更新...')
    await nextTick()
    
    console.log('📊 [服务列表] 开始初始化图表')
    tryInitChart()
    
    window.addEventListener('resize', resizeChart)
    
    // 监听容器尺寸变化
    if ('ResizeObserver' in window && serviceChartRef.value) {
      console.log('👁️ [服务列表] 启动ResizeObserver监听')
      resizeObserver = new ResizeObserver(() => {
        if (!serviceChart) {
          console.log('🔄 [ResizeObserver] 检测到尺寸变化，尝试初始化图表')
          tryInitChart()
        } else {
          console.log('📐 [ResizeObserver] 检测到尺寸变化，调整图表大小')
          resizeChart()
        }
      })
      resizeObserver.observe(serviceChartRef.value)
    } else {
      console.warn('⚠️ [服务列表] ResizeObserver不可用或容器未找到')
    }
    
    console.log('✅ [服务列表] 组件初始化完成')
  } catch (error) {
    console.error('❌ [服务列表] 初始加载服务列表失败:', error)
    handleError(error)
  } finally {
    pageLoading.value = false
  }
})

onUnmounted(() => {
  window.removeEventListener('resize', resizeChart)
  try { resizeObserver && resizeObserver.disconnect() } catch {}
  try { serviceChart && serviceChart.dispose() } catch {}
})

// 数据变化时更新图表
watch(() => systemStore.services.map(s => [s.name, s.tools_count]), (newVal, oldVal) => {
  console.log('🔄 [数据监听] 服务数据发生变化', { 
    新数据长度: newVal?.length, 
    旧数据长度: oldVal?.length 
  })
  if (!serviceChart) {
    console.log('📊 [数据监听] 图表未初始化，尝试初始化')
    tryInitChart()
  } else {
    console.log('📊 [数据监听] 图表已存在，重新渲染')
    renderServiceChart()
  }
}, { deep: true })
</script>

<style lang="scss" scoped>
.service-list {
  width: 92%;
  margin: 0 auto;
  max-width: none;
  .service-list__content {
    gap: 24px;
  }

  .page-header {
    margin-bottom: 0;
  }

  .page-header__actions {
    gap: 12px;
  }

  .stats-card {
    .stat-item {
      text-align: center;
      padding: 16px 0;

      .stat-value {
        font-size: 28px;
        font-weight: bold;
        color: var(--el-color-primary);
        margin-bottom: 4px;

        &.success {
          color: var(--el-color-success);
        }

        &.info {
          color: var(--el-color-info);
        }

        &.warning {
          color: var(--el-color-warning);
        }

        &.danger {
          color: var(--el-color-danger);
        }
      }

      .stat-label {
        font-size: 14px;
        color: var(--el-text-color-secondary);
      }
    }
  }

  .filter-card {
    .el-select,
    .el-input {
      width: 100%;
    }
  }

  .filter-action-btn {
    width: 100%;

    &__inner {
      width: 100%;
      justify-content: center;
    }
  }

  .table-card {
    .service-name {
      display: flex;
      align-items: center;
      gap: 8px;

      &.clickable {
        cursor: pointer;
        padding: 4px 8px;
        border-radius: 4px;
        transition: all 0.2s ease;

        &:hover {
          background-color: var(--el-color-primary-light-9);

          .service-name-text {
            color: var(--el-color-primary);
          }

          .view-tools-icon {
            opacity: 1;
            transform: translateX(0);
          }
        }
      }

      // 🔧 新增：服务状态指示器样式
      .service-status-indicator {
        position: relative;
        display: flex;
        align-items: center;

        .active-badge {
          position: absolute;
          top: -2px;
          right: -2px;
        }

        .config-badge {
          position: absolute;
          top: -2px;
          right: -2px;
        }
      }

      .service-name-content {
        flex: 1;
        display: flex;
        flex-direction: column;
        gap: 2px;

        .service-name-text {
          transition: color 0.2s ease;
          font-weight: 500;
        }

        .config-only-hint {
          font-size: 11px;
          color: var(--el-color-info);
          opacity: 0.8;
        }
      }

      .view-tools-icon {
        opacity: 0;
        transform: translateX(-8px);
        transition: all 0.2s ease;
        color: var(--el-color-primary);
        font-size: 14px;
      }

      .service-icon {
        &.local {
          color: var(--el-color-success);
        }

        &.remote {
          color: var(--el-color-info);
        }
      }
    }

    // 🔧 新增：生命周期详情样式
    .lifecycle-details {
      display: flex;
      flex-direction: column;
      gap: 4px;
      font-size: 12px;

      .lifecycle-stats {
        display: flex;
        gap: 4px;
        flex-wrap: wrap;
      }

      .last-ping {
        color: var(--el-color-info);
        font-size: 11px;
      }

      .error-message {
        margin-top: 2px;
      }
    }

    .config-only-info {
      display: flex;
      flex-direction: column;
      gap: 4px;
      align-items: flex-start;
    }

    .connection-info {
      .url,
      .command {
        font-weight: var(--font-weight-medium);
        margin-bottom: 2px;
      }

      .transport,
      .working-dir {
        font-size: var(--font-size-xs);
        color: var(--text-secondary);
      }
    }

    .tool-count-container {
      display: inline-block;
      cursor: pointer;
      padding: 4px;
      border-radius: 4px;
      transition: all 0.2s ease;

      &:hover {
        background-color: var(--el-color-primary-light-9);
        transform: scale(1.1);
      }
    }

    .tool-count-badge {
      &.clickable {
        transition: all 0.2s ease;
      }

      :deep(.el-badge__content) {
        top: 8px;
        right: 8px;
      }
    }

    .heartbeat-time {
      font-size: var(--font-size-sm);
      color: var(--text-regular);
    }

    .action-buttons {
      display: flex;
      gap: 6px;
      flex-wrap: nowrap;
      justify-content: flex-start;
      align-items: center;

      .action-btn {
        min-width: 60px;
        padding: 4px 8px;
        font-size: 12px;
        height: 28px;

        &.el-button--small {
          padding: 4px 8px;
        }
      }
    }
  }

  .service-details {
    .env-section {
      margin-top: 20px;

      h4 {
        margin-bottom: 12px;
        color: var(--text-primary);
      }
    }
  }

  /* 新布局 */
  .list-layout {
    align-items: stretch;
  }
  .list-card {
    display: flex;
    flex-direction: column;
    /* 固定卡片内容高度，与右侧图表一致 */
    :deep(.el-card__body) {
      height: 420px;
      display: flex;
      flex-direction: column;
    }
  }
  .list-toolbar {
    display: flex;
    gap: 12px;
    align-items: center;
  }
  .toolbar-input { width: 260px; }
  .toolbar-select { width: 160px; }
  .toolbar-spacer { flex: 1; }
  .table-wrap { margin-top: 8px; }

  .chart-card { 
    display: flex; 
    flex-direction: column; 
    height: 100%; 
  }
  .chart-card :deep(.el-card__body) { 
    height: 420px; 
    padding: 20px;
    display: flex;
    flex-direction: column;
  }
  .chart-content { 
    flex: 1; 
    display: flex; 
    min-height: 360px; /* 确保有最小高度 */
    width: 100%;
  }
  .chart-canvas { 
    flex: 1; 
    width: 100%; 
    height: 100%; 
    min-height: 360px; /* 确保有最小高度 */
  }
}

// 响应式适配
@include respond-to(xs) {
  .service-list {
    .page-header {
      flex-direction: column;
      align-items: flex-start;
      gap: 16px;
    }

    .page-header__actions {
      width: 100%;
      justify-content: flex-start;
    }

    .action-buttons {
      flex-direction: column;
      gap: 4px;

      .action-btn {
        width: 100%;
        min-width: auto;
      }
    }

    // 🔧 新增：连接状态样式
    .connection-status {
      .client-id {
        margin-bottom: 4px;
      }

      .connection-stats {
        display: flex;
        gap: 4px;
        margin-bottom: 4px;
        flex-wrap: wrap;
      }

      .state-time {
        font-size: 12px;
        color: var(--el-text-color-secondary);
      }
    }

    // 🔧 新增：错误信息样式
    .error-info {
      .error-tag {
        cursor: pointer;

        &:hover {
          opacity: 0.8;
        }
      }
    }

    .no-error, .not-active {
      display: flex;
      align-items: center;
      justify-content: center;
    }

    .text-muted {
      color: var(--el-text-color-disabled);
    }
  }

  // 编辑服务弹窗样式
  .edit-service-content {
    .edit-mode-selector {
      margin-bottom: 20px;
      text-align: center;
    }

    .fields-edit-mode {
      .client-id-display {
        margin-bottom: 20px;
        padding-bottom: 16px;
        border-bottom: 1px solid var(--el-border-color-lighter);

        .readonly-field {
          :deep(.el-input__inner) {
            background-color: var(--el-fill-color-lighter);
            color: var(--el-text-color-secondary);
            cursor: not-allowed;
          }

          .readonly-icon {
            color: var(--el-text-color-placeholder);
          }
        }
      }

      .edit-form {
        .form-field {
          margin-bottom: 16px;
        }

        .field-hint {
          font-size: 12px;
          color: var(--el-text-color-secondary);
          margin-top: 4px;
          line-height: 1.4;
        }
      }
    }

    .json-edit-mode {
      .json-actions {
        margin-top: 16px;
        display: flex;
        gap: 12px;
        justify-content: center;
      }
    }
  }
}
</style>
