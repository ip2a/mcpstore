<template>
  <div class="agent-detail">
    <!-- 页面头部 -->
    <div class="page-header">
      <div class="header-left">
        <el-button 
          :icon="ArrowLeft" 
          @click="$router.back()"
          class="back-btn"
        >
          返回
        </el-button>
        <div class="title-section">
          <h2 class="page-title">Agent详情</h2>
          <p class="page-description">{{ agentId }}</p>
        </div>
      </div>
      <div class="header-right">
        <el-button 
          type="primary" 
          :icon="Plus" 
          @click="addService"
        >
          添加服务
        </el-button>
        <el-button 
          :icon="Refresh" 
          @click="refreshData"
          :loading="loading"
        >
          刷新
        </el-button>
      </div>
    </div>

    <!-- Agent统计信息 -->
    <el-row :gutter="20" class="stats-row">
      <el-col :span="6">
        <el-card class="stat-card">
          <div class="stat-content">
            <div class="stat-value">{{ agentStats.services || 0 }}</div>
            <div class="stat-label">服务数量</div>
          </div>
        </el-card>
      </el-col>
      <el-col :span="6">
        <el-card class="stat-card">
          <div class="stat-content">
            <div class="stat-value">{{ agentStats.tools || 0 }}</div>
            <div class="stat-label">工具数量</div>
          </div>
        </el-card>
      </el-col>
      <el-col :span="6">
        <el-card class="stat-card">
          <div class="stat-content">
            <div class="stat-value">{{ agentStats.healthy_services || 0 }}</div>
            <div class="stat-label">健康服务</div>
          </div>
        </el-card>
      </el-col>
      <el-col :span="6">
        <el-card class="stat-card">
          <div class="stat-content">
            <div class="stat-value">{{ agentStats.total_tool_executions || 0 }}</div>
            <div class="stat-label">工具执行次数</div>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 服务列表 -->
    <el-card class="services-card">
      <template #header>
        <div class="card-header">
          <span>服务列表</span>
          <el-button 
            type="primary" 
            size="small"
            @click="addService"
          >
            添加服务
          </el-button>
        </div>
      </template>

      <div v-if="services.length === 0" class="empty-container">
        <el-empty description="暂无服务">
          <el-button type="primary" @click="addService">添加第一个服务</el-button>
        </el-empty>
      </div>

      <el-table v-else :data="services" style="width: 100%">
        <el-table-column prop="name" label="服务名称" width="200">
          <template #default="{ row }">
            <div class="service-name">
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

        <el-table-column label="连接信息" min-width="250">
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
            <el-tag :type="getServiceStatusType(row.status)" size="small">
              {{ getServiceStatusText(row.status) }}
            </el-tag>
          </template>
        </el-table-column>

        <el-table-column label="工具数" width="80" align="center">
          <template #default="{ row }">
            <el-badge
              :value="row.tool_count || 0"
              :max="99"
              class="tool-count-badge"
            >
              <el-icon><Tools /></el-icon>
            </el-badge>
          </template>
        </el-table-column>

        <el-table-column label="客户端ID" width="150">
          <template #default="{ row }">
            <el-tag size="small" type="info" v-if="row.client_id">
              {{ row.client_id.split('_').pop() }}
            </el-tag>
            <span v-else class="text-muted">N/A</span>
          </template>
        </el-table-column>

        <el-table-column label="操作" width="300">
          <template #default="{ row }">
            <el-button-group>
              <el-button size="small" @click="viewServiceTools(row)">
                查看工具
              </el-button>
              <el-button size="small" type="primary" @click="editService(row)">
                编辑
              </el-button>
              <el-button size="small" @click="restartService(row)">
                重启
              </el-button>
              <el-button size="small" type="danger" @click="deleteService(row)">
                删除
              </el-button>
            </el-button-group>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 工具列表 -->
    <el-card class="tools-card">
      <template #header>
        <span>可用工具</span>
      </template>

      <div v-if="tools.length === 0" class="empty-container">
        <el-empty description="暂无工具" />
      </div>

      <div v-else class="tools-grid">
        <div 
          v-for="tool in tools" 
          :key="tool.name"
          class="tool-card"
          @click="executeTool(tool)"
        >
          <div class="tool-header">
            <div class="tool-name">{{ tool.name }}</div>
            <div class="tool-service">{{ tool.service_name }}</div>
          </div>
          <div class="tool-description">
            {{ tool.description || '暂无描述' }}
          </div>
        </div>
      </div>
    </el-card>

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
</template>

<script setup>
import { ref, onMounted, computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import { ArrowLeft, Plus, Refresh, View, Setting, Check, FolderOpened, Link, Tools } from '@element-plus/icons-vue'
import { useAgentsStore } from '@/stores/agents'
import { SERVICE_STATUS_MAP, SERVICE_STATUS_COLORS } from '@/utils/constants'

const route = useRoute()
const router = useRouter()
const agentsStore = useAgentsStore()

// 响应式数据
const loading = ref(false)
const services = ref([])
const tools = ref([])
const agentStats = ref({})

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

// 计算属性
const agentId = computed(() => route.params.id)

// 计算属性：判断是否为远程服务
const isRemoteService = computed(() => {
  return editForm.value.url && !editForm.value.command
})

// 服务状态处理
const getServiceStatusType = (status) => {
  return SERVICE_STATUS_COLORS[status] || 'info'
}

const getServiceStatusText = (status) => {
  return SERVICE_STATUS_MAP[status] || '未知'
}

// 方法
const refreshData = async () => {
  loading.value = true
  try {
    await Promise.all([
      loadServices(),
      loadTools(),
      loadStats()
    ])
    ElMessage.success('数据刷新成功')
  } catch (error) {
    ElMessage.error('数据刷新失败: ' + (error.message || error))
  } finally {
    loading.value = false
  }
}

const loadServices = async () => {
  try {
    const data = await agentsStore.getAgentServices(agentId.value)
    services.value = data || []
  } catch (error) {
    console.error('加载服务列表失败:', error)
    services.value = []
  }
}

const loadTools = async () => {
  try {
    const data = await agentsStore.getAgentTools(agentId.value)
    tools.value = data || []
  } catch (error) {
    console.error('加载工具列表失败:', error)
    tools.value = []
  }
}

const loadStats = async () => {
  try {
    const data = await agentsStore.getAgentStats(agentId.value)
    agentStats.value = data || {}
  } catch (error) {
    console.error('加载统计信息失败:', error)
    agentStats.value = {}
  }
}

const addService = () => {
  router.push(`/agents/service-add?agentId=${agentId.value}`)
}

const viewServiceTools = (service) => {
  ElMessage.info(`查看服务 ${service.name} 的工具`)
}

const editService = async (service) => {
  try {
    editingService.value = service
    editMode.value = 'fields'

    // 获取服务配置
    const { agentServiceAPI } = await import('@/api/services')
    const response = await agentServiceAPI.showConfig(agentId.value)

    if (response.data.success) {
      // 从配置中找到当前服务的配置和client_id
      let serviceConfig = null
      let clientId = service.client_id || ''

      console.log('🔍 [DEBUG] Agent配置响应:', response.data.data)

      if (response.data.data.services) {
        const serviceInfo = response.data.data.services[service.name]
        serviceConfig = serviceInfo?.config
        clientId = serviceInfo?.client_id || clientId
        console.log('🔍 [DEBUG] 找到服务配置:', serviceConfig, 'Client ID:', clientId)
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
          isRemote: serviceConfig.url && !serviceConfig.command,
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

const restartService = async (service) => {
  try {
    // 优先使用client_id，如果没有则使用服务名
    const identifier = service.client_id || service.name
    console.log('🔍 [DEBUG] 重启服务:', { serviceName: service.name, clientId: service.client_id, identifier })

    await agentsStore.restartService(agentId.value, identifier)
    ElMessage.success(`服务 ${service.name} 重启成功`)
    await loadServices()
  } catch (error) {
    ElMessage.error(`服务 ${service.name} 重启失败: ${error.message}`)
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

    // 优先使用client_id，如果没有则使用服务名
    const identifier = service.client_id || service.name
    console.log('🔍 [DEBUG] 删除服务:', { serviceName: service.name, clientId: service.client_id, identifier })

    const { agentServiceAPI } = await import('@/api/services')
    const response = await agentServiceAPI.deleteConfig(agentId.value, identifier)

    if (response.data.success) {
      ElMessage.success(`服务 ${service.name} 删除成功`)
      await refreshData()
    } else {
      ElMessage.error(response.data.message || `服务 ${service.name} 删除失败`)
    }
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error(`服务 ${service.name} 删除失败: ${error.message}`)
    }
  }
}

const executeTool = (tool) => {
  router.push(`/tools/execute?agentId=${agentId.value}&toolName=${tool.name}&serviceName=${tool.service_name}`)
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

    const { agentServiceAPI } = await import('@/api/services')
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

    // 优先使用client_id，如果没有则使用服务名
    const identifier = editingService.value.client_id || editingService.value.name
    console.log('🔍 [DEBUG] 更新服务配置:', { serviceName: editingService.value.name, clientId: editingService.value.client_id, identifier, config })

    const response = await agentServiceAPI.updateConfigNew(agentId.value, identifier, config)

    if (response.data.success) {
      ElMessage.success('服务配置更新成功')
      editDialogVisible.value = false
      await refreshData()
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
  await refreshData()
})
</script>

<style lang="scss" scoped>
.agent-detail {
  .page-header {
    @include flex-between;
    margin-bottom: 20px;
    
    .header-left {
      display: flex;
      align-items: center;
      gap: 16px;
      
      .back-btn {
        padding: 8px 16px;
      }
      
      .title-section {
        .page-title {
          margin: 0 0 4px 0;
          font-size: 24px;
          font-weight: var(--font-weight-medium);
        }
        
        .page-description {
          margin: 0;
          color: var(--text-secondary);
          font-family: monospace;
        }
      }
    }
    
    .header-right {
      display: flex;
      gap: 12px;
    }
  }
  
  .stats-row {
    margin-bottom: 20px;
    
    .stat-card {
      .stat-content {
        text-align: center;
        
        .stat-value {
          font-size: 32px;
          font-weight: var(--font-weight-bold);
          color: var(--primary-color);
          margin-bottom: 8px;
        }
        
        .stat-label {
          color: var(--text-secondary);
          font-size: var(--font-size-sm);
        }
      }
    }
  }
  
  .services-card, .tools-card {
    margin-bottom: 20px;
    
    .card-header {
      @include flex-between;
      align-items: center;
    }
  }
  
  .tools-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 16px;
    
    .tool-card {
      @include card-shadow;
      padding: 16px;
      border-radius: var(--border-radius-base);
      cursor: pointer;
      transition: all 0.3s ease;
      
      &:hover {
        transform: translateY(-2px);
        box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
      }
      
      .tool-header {
        margin-bottom: 8px;
        
        .tool-name {
          font-weight: var(--font-weight-medium);
          margin-bottom: 4px;
        }
        
        .tool-service {
          font-size: var(--font-size-xs);
          color: var(--text-secondary);
        }
      }
      
      .tool-description {
        color: var(--text-regular);
        font-size: var(--font-size-sm);
      }
    }
  }
  
  .empty-container {
    text-align: center;
    padding: 40px 20px;
  }

  // 服务列表样式
  .service-name {
    display: flex;
    align-items: center;
    gap: 8px;

    .service-status-indicator {
      position: relative;
      display: flex;
      align-items: center;

      .service-icon {
        font-size: 16px;

        &.local {
          color: var(--el-color-success);
        }

        &.remote {
          color: var(--el-color-info);
        }
      }

      .active-badge, .config-badge {
        position: absolute;
        top: -2px;
        right: -2px;
      }
    }

    .service-name-content {
      display: flex;
      flex-direction: column;
      align-items: flex-start;

      .service-name-text {
        font-weight: 500;
      }

      .config-only-hint {
        font-size: 11px;
        color: var(--el-text-color-placeholder);
        background: var(--el-fill-color-lighter);
        padding: 1px 4px;
        border-radius: 2px;
      }
    }
  }

  .connection-info {
    .url, .command {
      font-family: var(--el-font-family-mono);
      font-size: 12px;
      color: var(--el-text-color-primary);
      margin-bottom: 2px;
    }

    .transport, .working-dir {
      font-size: 11px;
      color: var(--el-text-color-secondary);
    }
  }

  .tool-count-badge {
    cursor: pointer;

    &:hover {
      transform: scale(1.1);
    }
  }

  .text-muted {
    color: var(--el-text-color-disabled);
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
