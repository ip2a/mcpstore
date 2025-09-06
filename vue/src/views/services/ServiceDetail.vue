<template>
  <div class="service-detail">
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
    <div v-else v-loading="pageLoading" element-loading-text="加载服务详情...">
      <!-- 页面头部 -->
      <div class="page-header">
        <div class="header-left">
          <h2 class="page-title">
            <el-icon class="title-icon"><View /></el-icon>
            服务详情
          </h2>
          <p class="page-description">查看和管理服务的详细信息</p>
        </div>
        <div class="header-right">
          <el-button @click="$router.back()">
            <el-icon><ArrowLeft /></el-icon>
            返回
          </el-button>
        </div>
      </div>

      <!-- 主要内容 -->
      <div v-if="serviceData" class="main-content">
        <!-- 左侧：服务信息 -->
        <div class="left-panel">
          <!-- 基本信息 -->
          <el-card class="info-card">
            <template #header>
              <div class="card-header">
                <el-icon><InfoFilled /></el-icon>
                <span>基本信息</span>
                <div class="header-actions">
                  <el-button 
                    size="small" 
                    @click="toggleEdit('basic')"
                    :type="editMode.basic ? 'success' : 'primary'"
                    text
                  >
                    <el-icon><Edit /></el-icon>
                    {{ editMode.basic ? '保存' : '编辑' }}
                  </el-button>
                </div>
              </div>
            </template>

            <div class="info-section">
              <div class="info-item">
                <span class="info-label">服务名称</span>
                <span class="info-value">{{ serviceData.name }}</span>
              </div>
              
              <div class="info-item">
                <span class="info-label">服务类型</span>
                <el-tag :type="serviceData.command ? 'success' : 'info'">
                  {{ serviceData.command ? '本地服务' : '远程服务' }}
                </el-tag>
              </div>
              
              <div class="info-item">
                <span class="info-label">传输方式</span>
                <span class="info-value">{{ serviceData.transport || 'HTTP' }}</span>
              </div>
              
              <div class="info-item">
                <span class="info-label">状态</span>
                <el-tag :type="getStatusType(serviceData.status)">
                  {{ getStatusText(serviceData.status) }}
                </el-tag>
              </div>
              
              <div class="info-item">
                <span class="info-label">工具数量</span>
                <span class="info-value">{{ serviceData.tool_count || 0 }} 个</span>
              </div>
              
              <div class="info-item">
                <span class="info-label">客户端ID</span>
                <span class="info-value">{{ serviceData.client_id || '-' }}</span>
              </div>
            </div>
          </el-card>

          <!-- 连接信息 -->
          <el-card class="connection-card">
            <template #header>
              <div class="card-header">
                <el-icon><Connection /></el-icon>
                <span>连接信息</span>
                <div class="header-actions">
                  <el-button 
                    size="small" 
                    @click="toggleEdit('connection')"
                    :type="editMode.connection ? 'success' : 'primary'"
                    text
                  >
                    <el-icon><Edit /></el-icon>
                    {{ editMode.connection ? '保存' : '编辑' }}
                  </el-button>
                </div>
              </div>
            </template>

            <div class="connection-section">
              <!-- 远程服务连接信息 -->
              <div v-if="!serviceData.command" class="remote-info">
                <div class="info-item">
                  <span class="info-label">服务URL</span>
                  <div v-if="!editMode.connection" class="info-value">
                    {{ serviceData.url }}
                  </div>
                  <el-input 
                    v-else
                    v-model="editForm.url"
                    placeholder="请输入服务URL"
                    size="large"
                  />
                </div>
              </div>
              
              <!-- 本地服务连接信息 -->
              <div v-else class="local-info">
                <div class="info-item">
                  <span class="info-label">启动命令</span>
                  <div v-if="!editMode.connection" class="info-value">
                    {{ serviceData.command }}
                  </div>
                  <el-input 
                    v-else
                    v-model="editForm.command"
                    placeholder="请输入启动命令"
                    size="large"
                  />
                </div>
                
                <div class="info-item" v-if="serviceData.args && serviceData.args.length > 0">
                  <span class="info-label">命令参数</span>
                  <div v-if="!editMode.connection" class="info-value">
                    {{ serviceData.args.join(' ') }}
                  </div>
                  <el-input 
                    v-else
                    v-model="editForm.argsString"
                    placeholder="请输入命令参数（空格分隔）"
                    size="large"
                  />
                </div>
                
                <div class="info-item" v-if="serviceData.working_dir">
                  <span class="info-label">工作目录</span>
                  <div v-if="!editMode.connection" class="info-value">
                    {{ serviceData.working_dir }}
                  </div>
                  <el-input 
                    v-else
                    v-model="editForm.working_dir"
                    placeholder="请输入工作目录"
                    size="large"
                  />
                </div>
              </div>
            </div>
          </el-card>
        </div>

        <!-- 右侧：操作和工具 -->
        <div class="right-panel">
          <!-- 服务操作 -->
          <el-card class="actions-card">
            <template #header>
              <div class="card-header">
                <el-icon><Operation /></el-icon>
                <span>服务操作</span>
              </div>
            </template>

            <div class="actions-section">
              <el-button 
                type="success" 
                @click="restartService"
                :loading="actionLoading.restart"
                size="large"
                class="action-btn"
              >
                <el-icon><Refresh /></el-icon>
                重启服务
              </el-button>
              
              <el-button 
                type="warning" 
                @click="disconnectService"
                :loading="actionLoading.disconnect"
                size="large"
                class="action-btn"
              >
                <el-icon><SwitchButton /></el-icon>
                断开连接
              </el-button>
              
              <el-button 
                type="danger" 
                @click="deleteService"
                :loading="actionLoading.delete"
                size="large"
                class="action-btn"
              >
                <el-icon><Delete /></el-icon>
                删除服务
              </el-button>
            </div>
          </el-card>

          <!-- 服务工具 -->
          <el-card class="tools-card">
            <template #header>
              <div class="card-header">
                <el-icon><Tools /></el-icon>
                <span>服务工具</span>
                <span class="tool-count">{{ serviceTools.length }} 个</span>
              </div>
            </template>

            <div class="tools-section">
              <div v-if="serviceTools.length === 0" class="no-tools">
                <el-icon class="no-tools-icon"><Tools /></el-icon>
                <span>暂无工具</span>
              </div>
              
              <div v-else class="tools-list">
                <div 
                  v-for="tool in serviceTools" 
                  :key="tool.name"
                  class="tool-item"
                  @click="executeTool(tool)"
                >
                  <div class="tool-info">
                    <div class="tool-name">{{ tool.name }}</div>
                    <div class="tool-description">{{ tool.description || '暂无描述' }}</div>
                  </div>
                  <el-icon class="tool-arrow"><ArrowRight /></el-icon>
                </div>
              </div>
            </div>
          </el-card>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, reactive } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import { ElMessage, ElMessageBox } from 'element-plus'
import ErrorState from '@/components/common/ErrorState.vue'
import { SERVICE_STATUS_COLORS, SERVICE_STATUS_MAP } from '@/utils/constants'
import {
  View, ArrowLeft, InfoFilled, Edit, Connection, Operation, 
  Refresh, SwitchButton, Delete, Tools, ArrowRight
} from '@element-plus/icons-vue'

const route = useRoute()
const router = useRouter()
const systemStore = useSystemStore()

// 响应式数据
const pageLoading = ref(false)
const serviceData = ref(null)
const serviceTools = ref([])
const editMode = reactive({
  basic: false,
  connection: false
})
const editForm = reactive({
  url: '',
  command: '',
  argsString: '',
  working_dir: ''
})
const actionLoading = reactive({
  restart: false,
  disconnect: false,
  delete: false
})

// 错误状态
const hasError = ref(false)
const errorType = ref('network')
const errorTitle = ref('')
const errorDescription = ref('')
const errorDetails = ref('')
const showErrorDetails = ref(false)

// 计算属性
const serviceName = computed(() => route.params.serviceName)

// 状态处理函数
const getStatusType = (status) => {
  return SERVICE_STATUS_COLORS[status] || 'info'
}

const getStatusText = (status) => {
  return SERVICE_STATUS_MAP[status] || '未知'
}

// 方法
const fetchServiceDetail = async () => {
  try {
    pageLoading.value = true

    // 获取服务详情 - 使用现有的getServiceInfo方法
    const { api } = await import('@/api')
    const response = await api.store.getServiceInfo(serviceName.value)

    if (response.data.success) {
      serviceData.value = response.data.data

      // 初始化编辑表单
      initEditForm()

      // 获取服务工具
      await fetchServiceTools()
    } else {
      throw new Error(response.data.message || '获取服务详情失败')
    }
  } catch (error) {
    console.error('获取服务详情失败:', error)
    handleError(error)
  } finally {
    pageLoading.value = false
  }
}

const fetchServiceTools = async () => {
  try {
    await systemStore.fetchTools()
    serviceTools.value = systemStore.tools.filter(
      tool => tool.service_name === serviceName.value
    )
  } catch (error) {
    console.error('获取服务工具失败:', error)
  }
}

const initEditForm = () => {
  if (!serviceData.value) return

  editForm.url = serviceData.value.url || ''
  editForm.command = serviceData.value.command || ''
  editForm.argsString = (serviceData.value.args || []).join(' ')
  editForm.working_dir = serviceData.value.working_dir || ''
}

const toggleEdit = async (section) => {
  if (editMode[section]) {
    // 保存模式
    try {
      await saveChanges(section)
      editMode[section] = false
    } catch (error) {
      ElMessage.error('保存失败: ' + error.message)
    }
  } else {
    // 编辑模式
    editMode[section] = true
    initEditForm()
  }
}

const saveChanges = async (section) => {
  const { api } = await import('@/api')

  let updateData = {}

  if (section === 'connection') {
    if (serviceData.value.command) {
      // 本地服务
      updateData = {
        command: editForm.command,
        args: editForm.argsString.split(' ').filter(arg => arg.trim()),
        working_dir: editForm.working_dir
      }
    } else {
      // 远程服务
      updateData = {
        url: editForm.url
      }
    }
  }

  const response = await api.store.patchService(serviceName.value, updateData)

  if (response.data.success) {
    ElMessage.success('保存成功')
    await fetchServiceDetail()
  } else {
    throw new Error(response.data.message || '保存失败')
  }
}

const restartService = async () => {
  try {
    await ElMessageBox.confirm(
      `确定要重启服务 "${serviceName.value}" 吗？`,
      '重启确认',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )

    actionLoading.restart = true

    const { api } = await import('@/api')
    const response = await api.store.restartService(serviceName.value)

    if (response.data.success) {
      ElMessage.success('服务重启成功')
      await fetchServiceDetail()
    } else {
      ElMessage.error(response.data.message || '服务重启失败')
    }
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error('服务重启失败')
    }
  } finally {
    actionLoading.restart = false
  }
}

const disconnectService = async () => {
  try {
    await ElMessageBox.confirm(
      `确定要断开服务 "${serviceName.value}" 的连接吗？`,
      '断开确认',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )

    actionLoading.disconnect = true

    const { api } = await import('@/api')
    const response = await api.monitoring.gracefulDisconnect(serviceName.value)

    if (response.data.success) {
      ElMessage.success('服务断开成功')
      await fetchServiceDetail()
    } else {
      ElMessage.error(response.data.message || '服务断开失败')
    }
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error('服务断开失败')
    }
  } finally {
    actionLoading.disconnect = false
  }
}

const deleteService = async () => {
  try {
    await ElMessageBox.confirm(
      `确定要删除服务 "${serviceName.value}" 吗？此操作不可撤销！`,
      '删除确认',
      {
        confirmButtonText: '确定删除',
        cancelButtonText: '取消',
        type: 'error'
      }
    )

    actionLoading.delete = true

    const { api } = await import('@/api')
    const response = await api.store.deleteService(serviceName.value)

    if (response.data.success) {
      ElMessage.success('服务删除成功')
      router.push('/services/list')
    } else {
      ElMessage.error(response.data.message || '服务删除失败')
    }
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error('服务删除失败')
    }
  } finally {
    actionLoading.delete = false
  }
}

const executeTool = (tool) => {
  router.push({
    path: '/tools/execute',
    query: { tool: tool.name }
  })
}

const handleError = (error) => {
  hasError.value = true

  if (error.code === 'ECONNREFUSED' || error.code === 'ERR_NETWORK') {
    errorType.value = 'network'
    errorTitle.value = '网络连接失败'
    errorDescription.value = '无法连接到服务器，请检查网络连接或服务器状态'
  } else if (error.response?.status === 404) {
    errorType.value = 'not-found'
    errorTitle.value = '服务不存在'
    errorDescription.value = `服务 "${serviceName.value}" 不存在或已被删除`
  } else {
    errorType.value = 'server'
    errorTitle.value = '服务器错误'
    errorDescription.value = error.message || '获取服务详情时发生未知错误'
  }

  errorDetails.value = error.stack || error.toString()
  showErrorDetails.value = true
}

const handleRetry = async () => {
  hasError.value = false
  await fetchServiceDetail()
}

// 生命周期
onMounted(async () => {
  await fetchServiceDetail()
})
</script>

<style lang="scss" scoped>
.service-detail {
  .page-header {
    @include flex-between;
    margin-bottom: 24px;

    .header-left {
      .page-title {
        margin: 0 0 8px 0;
        font-size: 28px;
        font-weight: 600;
        display: flex;
        align-items: center;
        gap: 12px;

        .title-icon {
          font-size: 32px;
          color: var(--el-color-primary);
        }
      }

      .page-description {
        margin: 0;
        color: var(--el-text-color-secondary);
        font-size: 16px;
      }
    }
  }

  .main-content {
    display: grid;
    grid-template-columns: 1fr 400px;
    gap: 24px;
    align-items: start;
  }

  .left-panel {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .right-panel {
    display: flex;
    flex-direction: column;
    gap: 20px;
    position: sticky;
    top: 20px;
  }

  // 卡片头部样式
  .card-header {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 500;

    .header-actions {
      margin-left: auto;
    }

    .tool-count {
      margin-left: auto;
      font-size: 12px;
      color: var(--el-text-color-secondary);
    }
  }

  // 信息卡片样式
  .info-card, .connection-card {
    .info-section, .connection-section {
      .info-item {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 12px 0;
        border-bottom: 1px solid var(--el-border-color-lighter);

        &:last-child {
          border-bottom: none;
        }

        .info-label {
          font-weight: 500;
          color: var(--el-text-color-primary);
          min-width: 100px;
        }

        .info-value {
          color: var(--el-text-color-regular);
          word-break: break-all;
        }
      }
    }
  }

  // 操作卡片样式
  .actions-card {
    .actions-section {
      display: flex;
      flex-direction: column;
      gap: 12px;

      .action-btn {
        width: 100%;
        height: 48px;
        font-size: 16px;
        font-weight: 500;
      }
    }
  }

  // 工具卡片样式
  .tools-card {
    .tools-section {
      .no-tools {
        text-align: center;
        padding: 40px 20px;
        color: var(--el-text-color-secondary);

        .no-tools-icon {
          font-size: 48px;
          margin-bottom: 12px;
          display: block;
          color: var(--el-color-info);
        }
      }

      .tools-list {
        .tool-item {
          display: flex;
          align-items: center;
          padding: 16px;
          border-radius: 8px;
          cursor: pointer;
          transition: all 0.2s ease;
          border: 1px solid var(--el-border-color-lighter);
          margin-bottom: 8px;

          &:last-child {
            margin-bottom: 0;
          }

          &:hover {
            background: var(--el-fill-color-lighter);
            border-color: var(--el-color-primary);
            transform: translateY(-1px);
          }

          .tool-info {
            flex: 1;

            .tool-name {
              font-weight: 500;
              color: var(--el-text-color-primary);
              margin-bottom: 4px;
            }

            .tool-description {
              font-size: 12px;
              color: var(--el-text-color-secondary);
              line-height: 1.4;
            }
          }

          .tool-arrow {
            color: var(--el-text-color-placeholder);
            transition: all 0.2s ease;
          }

          &:hover .tool-arrow {
            color: var(--el-color-primary);
            transform: translateX(4px);
          }
        }
      }
    }
  }
}

// 响应式适配
@include respond-to(lg) {
  .service-detail {
    .main-content {
      grid-template-columns: 1fr 350px;
    }
  }
}

@include respond-to(md) {
  .service-detail {
    .main-content {
      grid-template-columns: 1fr;
      gap: 20px;
    }

    .right-panel {
      position: static;
    }
  }
}

@include respond-to(sm) {
  .service-detail {
    .page-header {
      flex-direction: column;
      align-items: flex-start;
      gap: 16px;

      .header-left .page-title {
        font-size: 24px;

        .title-icon {
          font-size: 28px;
        }
      }
    }

    .info-item {
      flex-direction: column;
      align-items: flex-start !important;
      gap: 8px;

      .info-label {
        min-width: auto !important;
      }
    }
  }
}
</style>
