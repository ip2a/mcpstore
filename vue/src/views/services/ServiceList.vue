<template>
  <div class="service-list">
    <!-- 错误状态 -->
    <ErrorState
      v-if="hasError && !pageLoading"
      :type="errorType"
      :title="errorTitle"
      :description="errorDescription"
      :show-details="showErrorDetails"
      :error-details="errorDetails"
      @retry="handleRetry"
    />

    <!-- 正常内容 -->
    <div v-else v-loading="pageLoading" element-loading-text="加载服务数据...">
      <!-- 页面头部 -->
    <div class="page-header">
      <div class="header-left">
        <h2 class="page-title">服务列表</h2>
        <p class="page-description">管理所有已注册的MCP服务</p>
      </div>
      <div class="header-right">
        <el-button 
          type="primary" 
          :icon="Plus" 
          @click="$router.push('/services/add')"
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
          <el-button type="warning" :icon="Tools">
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
    
    <!-- 筛选和搜索 -->
    <el-card class="filter-card">
      <el-row :gutter="20">
        <el-col :xs="24" :sm="12" :md="8">
          <el-input
            v-model="searchQuery"
            placeholder="搜索服务名称、URL或命令"
            :prefix-icon="Search"
            clearable
            @input="handleSearch"
          />
        </el-col>
        <el-col :xs="24" :sm="12" :md="6">
          <el-select
            v-model="statusFilter"
            placeholder="状态筛选"
            clearable
            @change="handleFilter"
          >
            <el-option label="全部状态" value="" />
            <el-option label="已激活服务" value="active" />
            <el-option label="仅配置服务" value="config-only" />
            <el-option label="健康" value="healthy" />
            <el-option label="初始化中" value="initializing" />
            <el-option label="重连中" value="reconnecting" />
            <el-option label="不可达" value="unreachable" />
            <el-option label="已断开" value="disconnected" />
          </el-select>
        </el-col>
        <el-col :xs="24" :sm="12" :md="6">
          <el-select 
            v-model="typeFilter" 
            placeholder="类型筛选"
            clearable
            @change="handleFilter"
          >
            <el-option label="全部类型" value="" />
            <el-option label="本地服务" value="local" />
            <el-option label="远程服务" value="remote" />
          </el-select>
        </el-col>
        <el-col :xs="24" :sm="12" :md="4">
          <el-dropdown @command="handleBatchAction" :disabled="selectedServices.length === 0">
            <el-button
              type="primary"
              :icon="Operation"
              :disabled="selectedServices.length === 0"
            >
              批量操作
              <el-icon class="el-icon--right"><ArrowDown /></el-icon>
            </el-button>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item command="batch-update" :icon="Edit">
                  批量更新
                </el-dropdown-item>
                <el-dropdown-item command="batch-restart" :icon="Refresh">
                  批量重启
                </el-dropdown-item>
                <el-dropdown-item command="batch-delete" :icon="Delete" divided>
                  批量删除
                </el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </el-col>
      </el-row>
    </el-card>
    
    <!-- 服务表格 -->
    <el-card class="table-card">
      <el-table
        v-loading="loading"
        :data="filteredServices"
        @selection-change="handleSelectionChange"
        stripe
        style="width: 100%"
      >
        <el-table-column type="selection" width="50" />
        
        <el-table-column prop="name" label="服务名称" width="200">
          <template #default="{ row }">
            <div class="service-name clickable" @click="viewServiceTools(row)">
              <!-- 🔧 改进：添加激活状态指示 -->
              <div class="service-status-indicator">
                <el-icon v-if="row.command" class="service-icon local">
                  <FolderOpened />
                </el-icon>
                <el-icon v-else class="service-icon remote">
                  <Link />
                </el-icon>
                <el-badge
                  v-if="row.is_active"
                  is-dot
                  class="active-badge"
                  type="success"
                />
                <el-badge
                  v-else
                  is-dot
                  class="config-badge"
                  type="info"
                />
              </div>
              <div class="service-name-content">
                <span class="service-name-text">{{ row.name }}</span>
                <span v-if="!row.is_active" class="config-only-hint">仅配置</span>
              </div>
              <el-icon class="view-tools-icon"><View /></el-icon>
            </div>
          </template>
        </el-table-column>
        
        <el-table-column label="类型" width="80">
          <template #default="{ row }">
            <el-tag 
              :type="row.command ? 'success' : 'info'"
              size="small"
            >
              {{ row.command ? '本地' : '远程' }}
            </el-tag>
          </template>
        </el-table-column>
        
        <el-table-column label="连接信息" min-width="300">
          <template #default="{ row }">
            <div v-if="row.url" class="connection-info">
              <div class="url">{{ row.url }}</div>
              <div class="transport">{{ row.transport || 'http' }}</div>
            </div>
            <div v-else-if="row.command" class="connection-info">
              <div class="command">{{ row.command }} {{ (row.args || []).join(' ') }}</div>
              <div class="working-dir" v-if="row.working_dir">{{ row.working_dir }}</div>
            </div>
          </template>
        </el-table-column>
        
        <el-table-column label="状态" width="80">
          <template #default="{ row }">
            <el-tag
              :type="getStatusType(row.status)"
              size="small"
            >
              {{ getStatusText(row.status) }}
            </el-tag>
          </template>
        </el-table-column>
        
        <el-table-column label="工具数" width="80" align="center">
          <template #default="{ row }">
            <div class="tool-count-container" @click="viewServiceTools(row)">
              <el-badge
                :value="row.tool_count || 0"
                :max="99"
                class="tool-count-badge clickable"
              >
                <el-icon><Tools /></el-icon>
              </el-badge>
            </div>
          </template>
        </el-table-column>
        
        <!-- 🔧 新增：生命周期状态详情 -->
        <el-table-column label="生命周期详情" width="200">
          <template #default="{ row }">
            <div v-if="row.is_active" class="lifecycle-details">
              <div class="lifecycle-stats">
                <el-tag size="small" type="success">
                  成功: {{ row.consecutive_successes || 0 }}
                </el-tag>
                <el-tag size="small" type="danger" v-if="row.consecutive_failures > 0">
                  失败: {{ row.consecutive_failures }}
                </el-tag>
              </div>
              <div v-if="row.last_ping_time" class="last-ping">
                最后检查: {{ formatTime(row.last_ping_time) }}
              </div>
              <div v-if="row.error_message" class="error-message">
                <el-tooltip :content="row.error_message" placement="top">
                  <el-tag size="small" type="danger">有错误</el-tag>
                </el-tooltip>
              </div>
            </div>
            <div v-else class="config-only-info">
              <el-tag size="small" type="info">未激活</el-tag>
              <el-button
                size="small"
                type="primary"
                link
                @click="activateService(row)"
                :loading="row.activating"
              >
                激活服务
              </el-button>
            </div>
          </template>
        </el-table-column>
        
        <el-table-column label="操作" width="300" fixed="right">
          <template #default="{ row }">
            <div class="action-buttons">
              <el-button
                size="small"
                type="primary"
                @click="viewServiceDetails(row)"
                class="action-btn"
              >
                详情
              </el-button>
              <el-button
                size="small"
                type="success"
                @click="editService(row)"
                class="action-btn"
              >
                编辑
              </el-button>
              <el-button
                size="small"
                type="warning"
                @click="restartService(row)"
                :loading="row.restarting"
                class="action-btn"
              >
                重启
              </el-button>
              <el-button
                size="small"
                type="danger"
                @click="deleteService(row)"
                class="action-btn"
              >
                删除
              </el-button>
            </div>
          </template>
        </el-table-column>
      </el-table>
      
      <!-- 空状态 -->
      <div v-if="filteredServices.length === 0 && !loading" class="empty-container">
        <el-icon class="empty-icon"><Connection /></el-icon>
        <div class="empty-text">暂无服务</div>
        <div class="empty-description">
          {{ searchQuery || statusFilter || typeFilter ? '没有找到匹配的服务' : '还没有注册任何服务' }}
        </div>
        <el-button 
          v-if="!searchQuery && !statusFilter && !typeFilter"
          type="primary" 
          @click="$router.push('/services/add')"
        >
          添加第一个服务
        </el-button>
      </div>
    </el-card>
    
    <!-- 批量更新对话框 -->
    <BatchUpdateDialog
      v-model="batchUpdateDialogVisible"
      :services="selectedServices"
      @updated="handleBatchUpdateSuccess"
    />

    <!-- 服务详情对话框 -->
    <el-dialog
      v-model="detailDialogVisible"
      :title="`服务详情 - ${selectedService?.name}`"
      width="600px"
    >
      <div v-if="selectedService" class="service-details">
        <el-descriptions :column="1" border>
          <el-descriptions-item label="服务名称">
            {{ selectedService.name }}
          </el-descriptions-item>
          <el-descriptions-item label="服务类型">
            <el-tag :type="selectedService.command ? 'success' : 'info'">
              {{ selectedService.command ? '本地服务' : '远程服务' }}
            </el-tag>
          </el-descriptions-item>
          <el-descriptions-item label="状态">
            <el-tag :type="selectedService.status === 'healthy' ? 'success' : 'danger'">
              {{ selectedService.status === 'healthy' ? '健康' : '异常' }}
            </el-tag>
          </el-descriptions-item>
          <el-descriptions-item v-if="selectedService.url" label="URL">
            {{ selectedService.url }}
          </el-descriptions-item>
          <el-descriptions-item v-if="selectedService.command" label="命令">
            {{ selectedService.command }} {{ (selectedService.args || []).join(' ') }}
          </el-descriptions-item>
          <el-descriptions-item v-if="selectedService.working_dir" label="工作目录">
            {{ selectedService.working_dir }}
          </el-descriptions-item>
          <el-descriptions-item label="传输类型">
            {{ selectedService.transport_type || 'http' }}
          </el-descriptions-item>
          <el-descriptions-item label="工具数量">
            {{ selectedService.tool_count || 0 }}
          </el-descriptions-item>
          <el-descriptions-item v-if="selectedService.last_heartbeat" label="最后心跳">
            {{ formatTime(selectedService.last_heartbeat) }}
          </el-descriptions-item>
        </el-descriptions>
        
        <!-- 环境变量 -->
        <div v-if="selectedService.env && Object.keys(selectedService.env).length > 0" class="env-section">
          <h4>环境变量</h4>
          <el-table :data="envTableData" size="small">
            <el-table-column prop="key" label="变量名" />
            <el-table-column prop="value" label="值" />
          </el-table>
        </div>
      </div>
      
      <template #footer>
        <el-button @click="detailDialogVisible = false">关闭</el-button>
        <el-button type="success" @click="editService(selectedService)">编辑服务</el-button>
        <el-button type="warning" @click="restartService(selectedService)">重启服务</el-button>
      </template>
    </el-dialog>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import { ElMessage, ElMessageBox } from 'element-plus'
import dayjs from 'dayjs'
import BatchUpdateDialog from './BatchUpdateDialog.vue'
import ErrorState from '@/components/common/ErrorState.vue'
import { SERVICE_STATUS_COLORS, SERVICE_STATUS_MAP } from '@/utils/constants'
import {
  Plus, Refresh, Search, Delete, Connection, FolderOpened,
  Link, Tools, View, ArrowDown, RefreshLeft, Setting, Operation, Edit
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
const selectedServices = ref([])
const detailDialogVisible = ref(false)
const selectedService = ref(null)
const batchUpdateDialogVisible = ref(false)

// 错误状态
const hasError = ref(false)
const errorType = ref('network')
const errorTitle = ref('')
const errorDescription = ref('')
const errorDetails = ref('')
const showErrorDetails = ref(false)

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
  
  // 类型过滤
  if (typeFilter.value) {
    services = services.filter(service => {
      if (typeFilter.value === 'local') {
        return !!service.command
      } else if (typeFilter.value === 'remote') {
        return !!service.url
      }
      return true
    })
  }
  
  return services
})

const envTableData = computed(() => {
  if (!selectedService.value?.env) return []
  return Object.entries(selectedService.value.env).map(([key, value]) => ({
    key,
    value: key.toLowerCase().includes('password') || key.toLowerCase().includes('key') 
      ? '***' : value
  }))
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
    await systemStore.fetchServices()
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
}

const handleBatchAction = async (command) => {
  if (selectedServices.value.length === 0) return

  switch (command) {
    case 'batch-update':
      await handleBatchUpdate()
      break
    case 'batch-restart':
      await handleBatchRestart()
      break
    case 'batch-delete':
      await handleBatchDelete()
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

    const { storeServiceAPI } = await import('@/api/services')
    const serviceNames = selectedServices.value.map(s => s.name)
    const response = await storeServiceAPI.batchRestartServices(serviceNames)

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

const handleBatchDelete = async () => {
  if (selectedServices.value.length === 0) return

  try {
    await ElMessageBox.confirm(
      `确定要删除选中的 ${selectedServices.value.length} 个服务吗？`,
      '批量删除确认',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )

    const { storeServiceAPI } = await import('@/api/services')
    const serviceNames = selectedServices.value.map(s => s.name)
    const response = await storeServiceAPI.batchDeleteServices(serviceNames)

    if (response.data.success) {
      ElMessage.success('批量删除成功')
      await refreshServices()
    } else {
      ElMessage.error(response.data.message || '批量删除失败')
    }

    selectedServices.value = []
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error('批量删除失败')
    }
  }
}

const viewServiceTools = (service) => {
  // 跳转到工具列表页面，并筛选该服务的工具
  router.push({
    path: '/tools/list',
    query: { service: service.name }
  })
}

const viewServiceDetails = (service) => {
  selectedService.value = service
  detailDialogVisible.value = true
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
    
    await systemStore.deleteService(service.name)
    ElMessage.success(`服务 ${service.name} 删除成功`)
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error(`服务 ${service.name} 删除失败`)
    }
  }
}

const editService = (service) => {
  // 跳转到编辑页面
  router.push({
    path: `/services/edit/${service.name}`,
    query: route.query.agent ? { agent: route.query.agent } : {}
  })
}

const formatTime = (time) => {
  return dayjs(time).format('YYYY-MM-DD HH:mm:ss')
}

// 🔧 新增：服务激活功能
const activateService = async (service) => {
  try {
    service.activating = true

    const { storeServiceAPI } = await import('@/api/services')
    const response = await storeServiceAPI.activateService(service.name)

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

    const { storeServiceAPI } = await import('@/api/services')
    const response = await storeServiceAPI.resetConfig()

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

// 错误处理函数
const handleError = (error) => {
  hasError.value = true

  if (error.code === 'ECONNREFUSED' || error.code === 'ERR_NETWORK') {
    errorType.value = 'network'
    errorTitle.value = '无法连接到后端服务'
    errorDescription.value = '请检查后端服务是否正常运行，或稍后重试'
  } else if (error.response?.status >= 500) {
    errorType.value = 'server'
    errorTitle.value = '服务器内部错误'
    errorDescription.value = '服务器遇到了问题，请稍后重试'
  } else if (error.code === 'ECONNABORTED' || error.message?.includes('timeout')) {
    errorType.value = 'network'
    errorTitle.value = '请求超时'
    errorDescription.value = '网络连接超时，请检查网络状况或稍后重试'
  } else {
    errorType.value = 'unknown'
    errorTitle.value = '加载失败'
    errorDescription.value = '服务列表加载失败，请稍后重试'
  }

  // 显示错误详情（开发环境）
  if (import.meta.env.DEV) {
    showErrorDetails.value = true
    errorDetails.value = `错误类型: ${error.name || 'Unknown'}
错误消息: ${error.message || '无详细信息'}
错误代码: ${error.code || 'N/A'}
状态码: ${error.response?.status || 'N/A'}`
  }
}

// 重试处理
const handleRetry = async () => {
  pageLoading.value = true
  hasError.value = false
  try {
    await systemStore.fetchServices()
  } catch (error) {
    handleError(error)
  } finally {
    pageLoading.value = false
  }
}

// 生命周期
onMounted(async () => {
  pageLoading.value = true
  try {
    await systemStore.fetchServices()
  } catch (error) {
    console.error('初始加载服务列表失败:', error)
    handleError(error)
  } finally {
    pageLoading.value = false
  }
})
</script>

<style lang="scss" scoped>
.service-list {
  .page-header {
    @include flex-between;
    margin-bottom: 20px;
    
    .header-left {
      .page-title {
        margin: 0 0 4px 0;
        font-size: 24px;
        font-weight: var(--font-weight-medium);
      }
      
      .page-description {
        margin: 0;
        color: var(--text-secondary);
      }
    }
    
    .header-right {
      display: flex;
      gap: 12px;
    }
  }
  
  .filter-card {
    margin-bottom: 20px;
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
}

// 响应式适配
@include respond-to(xs) {
  .service-list {
    .page-header {
      flex-direction: column;
      align-items: flex-start;
      gap: 16px;
      
      .header-right {
        width: 100%;
        justify-content: flex-end;
      }
    }
    
    .action-buttons {
      flex-direction: column;
      gap: 4px;

      .action-btn {
        width: 100%;
        min-width: auto;
      }
    }
  }
}
</style>
