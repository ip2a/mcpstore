<template>
  <div class="reset-manager">
    <!-- 页面头部 -->
    <div class="page-header">
      <div class="header-left">
        <h2 class="page-title">重置管理</h2>
        <p class="page-description">管理系统配置重置功能和配置查看</p>
      </div>
      <div class="header-right">
        <el-button
          :icon="Refresh"
          @click="refreshData"
          :loading="loading"
        >
          刷新数据
        </el-button>
      </div>
    </div>

    <!-- 重置功能卡片 -->
    <el-row :gutter="16" class="reset-cards">
      <!-- 配置重置功能 -->
      <el-col :xs="24" :lg="24">
        <el-card class="reset-card">
          <template #header>
            <div class="card-header">
              <el-icon class="header-icon config"><Setting /></el-icon>
              <span>配置重置功能</span>
            </div>
          </template>

          <div class="reset-section">
            <p class="section-description">
              重置系统配置，包括内存数据和文件配置的清理。
            </p>

            <el-row :gutter="16" class="reset-items">
              <!-- 重置所有配置 -->
              <el-col :xs="24" :md="8">
                <div class="reset-item">
                  <div class="item-info">
                    <h4>重置所有配置</h4>
                    <p>重置所有Agent和Store的配置</p>
                    <el-tag size="small" type="danger">全局重置</el-tag>
                  </div>
                  <el-button
                    type="danger"
                    size="small"
                    :loading="resetLoading.allConfig"
                    @click="resetAllConfig"
                  >
                    重置所有
                  </el-button>
                </div>
              </el-col>

              <!-- 重置Store配置 -->
              <el-col :xs="24" :md="8">
                <div class="reset-item">
                  <div class="item-info">
                    <h4>重置Store配置</h4>
                    <p>重置主Store的服务和配置</p>
                    <el-tag size="small" type="warning">Store重置</el-tag>
                  </div>
                  <el-button
                    type="danger"
                    size="small"
                    :loading="resetLoading.storeConfig"
                    @click="resetStoreConfig"
                  >
                    重置Store
                  </el-button>
                </div>
              </el-col>

              <!-- 根据Agent ID重置 -->
              <el-col :xs="24" :md="8">
                <div class="reset-item">
                  <div class="item-info">
                    <h4>根据Agent ID重置</h4>
                    <p>重置指定Agent的配置</p>
                    <div class="agent-selector">
                      <el-input
                        v-model="selectedAgentId"
                        placeholder="输入Agent ID"
                        size="small"
                        style="margin-top: 8px;"
                      />
                    </div>
                  </div>
                  <el-button
                    type="danger"
                    size="small"
                    :loading="resetLoading.agentConfig"
                    :disabled="!selectedAgentId"
                    @click="resetAgentConfig"
                  >
                    重置Agent
                  </el-button>
                </div>
              </el-col>
            </el-row>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 配置展示 -->
    <el-card class="config-display-card">
      <template #header>
        <div class="card-header">
          <el-icon class="header-icon config"><Document /></el-icon>
          <span>配置展示</span>
          <div class="header-actions">
            <el-select
              v-model="configScope"
              @change="loadConfig"
              size="small"
              style="width: 200px; margin-right: 12px;"
            >
              <el-option label="所有配置" value="all" />
              <el-option label="Store配置" value="global_agent_store" />
            </el-select>
            <el-input
              v-if="configScope === 'agent'"
              v-model="configAgentId"
              placeholder="Agent ID"
              size="small"
              style="width: 150px; margin-right: 12px;"
              @keyup.enter="loadConfig"
            />
            <el-button
              size="small"
              :icon="Refresh"
              @click="loadConfig"
              :loading="configLoading"
            >
              刷新
            </el-button>
          </div>
        </div>
      </template>

      <div class="config-content">
        <div v-if="configLoading" class="loading-state">
          <el-icon class="is-loading"><Refresh /></el-icon>
          <span>加载配置中...</span>
        </div>

        <div v-else-if="configError" class="error-state">
          <el-alert
            :title="configError"
            type="error"
            show-icon
            :closable="false"
          />
        </div>

        <div v-else-if="configData" class="config-data">
          <!-- 配置统计 -->
          <div class="config-summary" v-if="configData.summary">
            <el-row :gutter="16">
              <el-col :span="8" v-if="configData.summary.total_agents">
                <div class="summary-item">
                  <div class="summary-label">总Agent数</div>
                  <div class="summary-value">{{ configData.summary.total_agents }}</div>
                </div>
              </el-col>
              <el-col :span="8">
                <div class="summary-item">
                  <div class="summary-label">总服务数</div>
                  <div class="summary-value">{{ configData.summary.total_services }}</div>
                </div>
              </el-col>
              <el-col :span="8">
                <div class="summary-item">
                  <div class="summary-label">总客户端数</div>
                  <div class="summary-value">{{ configData.summary.total_clients }}</div>
                </div>
              </el-col>
            </el-row>
          </div>

          <!-- 配置详情 -->
          <div class="config-details">
            <el-collapse v-model="activeConfigPanels">
              <!-- 显示所有Agent配置 -->
              <template v-if="configData.agents">
                <el-collapse-item
                  v-for="(agentData, agentId) in configData.agents"
                  :key="agentId"
                  :name="agentId"
                >
                  <template #title>
                    <div class="agent-title">
                      <el-icon><User /></el-icon>
                      <span>{{ agentId }}</span>
                      <el-tag size="small" type="info">
                        {{ Object.keys(agentData.services || {}).length }} 个服务
                      </el-tag>
                    </div>
                  </template>

                  <div class="agent-services">
                    <div
                      v-for="(serviceInfo, serviceName) in agentData.services"
                      :key="serviceName"
                      class="service-item"
                    >
                      <div class="service-header">
                        <h4>{{ serviceName }}</h4>
                        <el-tag size="small">{{ serviceInfo.client_id }}</el-tag>
                      </div>
                      <pre class="service-config">{{ JSON.stringify(serviceInfo.config, null, 2) }}</pre>
                    </div>
                  </div>
                </el-collapse-item>
              </template>

              <!-- 显示单个Agent配置 -->
              <template v-else-if="configData.agent_id">
                <el-collapse-item name="single-agent" title="Agent配置">
                  <div class="agent-services">
                    <div
                      v-for="(serviceInfo, serviceName) in configData.services"
                      :key="serviceName"
                      class="service-item"
                    >
                      <div class="service-header">
                        <h4>{{ serviceName }}</h4>
                        <el-tag size="small">{{ serviceInfo.client_id }}</el-tag>
                      </div>
                      <pre class="service-config">{{ JSON.stringify(serviceInfo.config, null, 2) }}</pre>
                    </div>
                  </div>
                </el-collapse-item>
              </template>
            </el-collapse>
          </div>
        </div>

        <div v-else class="empty-state">
          <el-empty description="暂无配置数据" />
        </div>
      </div>
    </el-card>

    <!-- 系统状态 -->
    <el-card class="status-card">
      <template #header>
        <div class="card-header">
          <el-icon class="header-icon status"><Monitor /></el-icon>
          <span>系统状态</span>
        </div>
      </template>

      <el-row :gutter="16">
        <el-col :xs="24" :sm="8">
          <div class="status-item">
            <div class="status-label">总服务数</div>
            <div class="status-value">{{ systemStats.totalServices || 0 }}</div>
          </div>
        </el-col>
        <el-col :xs="24" :sm="8">
          <div class="status-item">
            <div class="status-label">健康服务</div>
            <div class="status-value success">{{ systemStats.healthyServices || 0 }}</div>
          </div>
        </el-col>
        <el-col :xs="24" :sm="8">
          <div class="status-item">
            <div class="status-label">可用工具</div>
            <div class="status-value">{{ systemStats.totalTools || 0 }}</div>
          </div>
        </el-col>
      </el-row>
    </el-card>

    <!-- 操作日志 -->
    <el-card class="log-card">
      <template #header>
        <div class="card-header">
          <el-icon class="header-icon log"><Document /></el-icon>
          <span>操作日志</span>
          <el-button 
            size="small" 
            text 
            @click="clearLogs"
          >
            清空日志
          </el-button>
        </div>
      </template>

      <div class="log-container">
        <div 
          v-for="(log, index) in operationLogs" 
          :key="index"
          class="log-item"
          :class="log.type"
        >
          <span class="log-time">{{ log.time }}</span>
          <span class="log-message">{{ log.message }}</span>
        </div>
        <div v-if="operationLogs.length === 0" class="no-logs">
          暂无操作日志
        </div>
      </div>
    </el-card>
  </div>
</template>

<script setup>
import { ref, reactive, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  Refresh, Document, Setting, Monitor, User
} from '@element-plus/icons-vue'
import { api } from '@/api'

// 响应式数据
const loading = ref(false)
const selectedAgentId = ref('')
const systemStats = reactive({
  totalServices: 0,
  healthyServices: 0,
  totalTools: 0
})

const resetLoading = reactive({
  allConfig: false,
  storeConfig: false,
  agentConfig: false
})

const operationLogs = ref([])

// 配置展示相关数据
const configScope = ref('all')
const configAgentId = ref('')
const configLoading = ref(false)
const configError = ref('')
const configData = ref(null)
const activeConfigPanels = ref([])

// 添加日志
const addLog = (message, type = 'info') => {
  const now = new Date()
  const time = now.toLocaleTimeString()
  operationLogs.value.unshift({
    time,
    message,
    type
  })
  
  // 限制日志数量
  if (operationLogs.value.length > 50) {
    operationLogs.value = operationLogs.value.slice(0, 50)
  }
}

// 清空日志
const clearLogs = () => {
  operationLogs.value = []
  addLog('日志已清空')
}

// 刷新数据
const refreshData = async () => {
  loading.value = true
  try {
    const response = await api.store.getStats()
    if (response.data.success) {
      const stats = response.data.data
      systemStats.totalServices = stats.services?.total || 0
      systemStats.healthyServices = stats.services?.healthy || 0
      systemStats.totalTools = stats.tools?.total || 0
    }
    addLog('系统状态已刷新')
  } catch (error) {
    ElMessage.error('获取系统状态失败')
    addLog(`获取系统状态失败: ${error.message}`, 'error')
  } finally {
    loading.value = false
  }
}



// 重置所有配置
const resetAllConfig = async () => {
  try {
    await ElMessageBox.confirm(
      '此操作将重置所有Agent和Store的配置，包括内存数据和文件配置。是否继续？',
      '确认重置所有配置',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )

    resetLoading.allConfig = true
    const response = await api.store.resetConfig('all')

    if (response.data.success) {
      ElMessage.success('所有配置重置成功')
      addLog('所有配置重置成功', 'success')
      await refreshData()
      await loadConfig()
    } else {
      ElMessage.error(response.data.message || '所有配置重置失败')
      addLog(`所有配置重置失败: ${response.data.message}`, 'error')
    }
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error('所有配置重置失败')
      addLog(`所有配置重置失败: ${error.message}`, 'error')
    }
  } finally {
    resetLoading.allConfig = false
  }
}

// 重置Store配置
const resetStoreConfig = async () => {
  try {
    await ElMessageBox.confirm(
      '此操作将重置Store的服务和配置，包括内存数据和文件配置。是否继续？',
      '确认重置Store配置',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )

    resetLoading.storeConfig = true
    const response = await api.store.resetConfig('global_agent_store')

    if (response.data.success) {
      ElMessage.success('Store配置重置成功')
      addLog('Store配置重置成功', 'success')
      await refreshData()
      await loadConfig()
    } else {
      ElMessage.error(response.data.message || 'Store配置重置失败')
      addLog(`Store配置重置失败: ${response.data.message}`, 'error')
    }
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error('Store配置重置失败')
      addLog(`Store配置重置失败: ${error.message}`, 'error')
    }
  } finally {
    resetLoading.storeConfig = false
  }
}

// 重置Agent配置
const resetAgentConfig = async () => {
  if (!selectedAgentId.value.trim()) {
    ElMessage.warning('请输入Agent ID')
    return
  }

  try {
    await ElMessageBox.confirm(
      `此操作将重置Agent "${selectedAgentId.value}" 的所有配置，包括内存数据和文件配置。是否继续？`,
      '确认重置Agent配置',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )

    resetLoading.agentConfig = true
    const response = await api.agent.resetConfig(selectedAgentId.value)

    if (response.data.success) {
      ElMessage.success(`Agent "${selectedAgentId.value}" 配置重置成功`)
      addLog(`Agent "${selectedAgentId.value}" 配置重置成功`, 'success')
      selectedAgentId.value = ''
      await refreshData()
      await loadConfig()
    } else {
      ElMessage.error(response.data.message || 'Agent配置重置失败')
      addLog(`Agent "${selectedAgentId.value}" 配置重置失败: ${response.data.message}`, 'error')
    }
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error('Agent配置重置失败')
      addLog(`Agent "${selectedAgentId.value}" 配置重置失败: ${error.message}`, 'error')
    }
  } finally {
    resetLoading.agentConfig = false
  }
}

// 配置加载方法
const loadConfig = async () => {
  configLoading.value = true
  configError.value = ''
  configData.value = null

  try {
    let response

    if (configScope.value === 'agent') {
      if (!configAgentId.value.trim()) {
        configError.value = '请输入Agent ID'
        return
      }
      response = await api.agent.showConfig(configAgentId.value)
    } else {
      response = await api.store.showConfig(configScope.value)
    }

    if (response.data.success) {
      configData.value = response.data.data
      addLog(`配置加载成功: ${configScope.value}`, 'success')

      // 自动展开第一个面板
      if (configData.value.agents) {
        const firstAgentId = Object.keys(configData.value.agents)[0]
        if (firstAgentId) {
          activeConfigPanels.value = [firstAgentId]
        }
      } else if (configData.value.agent_id) {
        activeConfigPanels.value = ['single-agent']
      }
    } else {
      configError.value = response.data.message || '加载配置失败'
      addLog(`配置加载失败: ${configError.value}`, 'error')
    }
  } catch (error) {
    configError.value = `加载配置失败: ${error.message}`
    addLog(`配置加载失败: ${error.message}`, 'error')
  } finally {
    configLoading.value = false
  }
}

// 生命周期
onMounted(() => {
  refreshData()
  loadConfig()
  addLog('重置管理页面已加载')
})
</script>

<style lang="scss" scoped>
.reset-manager {
  padding: 20px;

  .page-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 20px;

    .header-left {
      .page-title {
        margin: 0 0 8px 0;
        font-size: 24px;
        font-weight: 600;
        color: var(--el-text-color-primary);
      }

      .page-description {
        margin: 0;
        color: var(--el-text-color-regular);
        font-size: 14px;
      }
    }
  }

  .reset-cards {
    margin-bottom: 20px;

    .reset-card {
      height: 100%;

      .card-header {
        display: flex;
        align-items: center;
        gap: 8px;
        font-weight: 600;

        .header-icon {
          font-size: 18px;

          &.file {
            color: var(--el-color-primary);
          }

          &.config {
            color: var(--el-color-success);
          }

          &.status {
            color: var(--el-color-info);
          }

          &.log {
            color: var(--el-color-warning);
          }
        }
      }

      .reset-section {
        .section-description {
          margin: 0 0 20px 0;
          color: var(--el-text-color-regular);
          font-size: 14px;
          line-height: 1.5;
        }

        .reset-items {
          .reset-item {
            display: flex;
            justify-content: space-between;
            align-items: flex-start;
            padding: 16px;
            border: 1px solid var(--el-border-color-lighter);
            border-radius: 8px;
            margin-bottom: 12px;
            transition: all 0.2s ease;

            &:hover {
              border-color: var(--el-border-color);
              box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
            }

            &:last-child {
              margin-bottom: 0;
            }

            .item-info {
              flex: 1;

              h4 {
                margin: 0 0 4px 0;
                font-size: 16px;
                font-weight: 600;
                color: var(--el-text-color-primary);
              }

              p {
                margin: 0 0 8px 0;
                color: var(--el-text-color-regular);
                font-size: 14px;
              }

              .item-tags {
                display: flex;
                gap: 8px;
                flex-wrap: wrap;
              }

              .agent-selector {
                margin-top: 8px;
              }
            }
          }
        }
      }
    }
  }

  .status-card {
    margin-bottom: 20px;

    .status-item {
      text-align: center;
      padding: 16px;

      .status-label {
        font-size: 14px;
        color: var(--el-text-color-regular);
        margin-bottom: 8px;
      }

      .status-value {
        font-size: 24px;
        font-weight: 600;
        color: var(--el-text-color-primary);

        &.success {
          color: var(--el-color-success);
        }
      }
    }
  }

  .log-card {
    .log-container {
      max-height: 300px;
      overflow-y: auto;

      .log-item {
        display: flex;
        gap: 12px;
        padding: 8px 12px;
        border-radius: 4px;
        margin-bottom: 4px;
        font-size: 14px;

        &.info {
          background-color: var(--el-color-info-light-9);
          border-left: 3px solid var(--el-color-info);
        }

        &.success {
          background-color: var(--el-color-success-light-9);
          border-left: 3px solid var(--el-color-success);
        }

        &.error {
          background-color: var(--el-color-danger-light-9);
          border-left: 3px solid var(--el-color-danger);
        }

        .log-time {
          color: var(--el-text-color-secondary);
          font-family: monospace;
          white-space: nowrap;
        }

        .log-message {
          color: var(--el-text-color-primary);
          word-break: break-word;
        }
      }

      .no-logs {
        text-align: center;
        color: var(--el-text-color-placeholder);
        padding: 40px;
        font-style: italic;
      }
    }
  }

  // 配置展示卡片样式
  .config-display-card {
    margin-bottom: 20px;

    .card-header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 8px;
      font-weight: 600;

      .header-actions {
        display: flex;
        align-items: center;
      }
    }

    .config-content {
      .loading-state {
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 8px;
        padding: 40px;
        color: var(--el-text-color-secondary);
      }

      .error-state {
        padding: 20px;
      }

      .empty-state {
        padding: 40px;
      }

      .config-summary {
        margin-bottom: 20px;
        padding: 16px;
        background: var(--el-fill-color-lighter);
        border-radius: 8px;

        .summary-item {
          text-align: center;

          .summary-label {
            font-size: 12px;
            color: var(--el-text-color-secondary);
            margin-bottom: 4px;
          }

          .summary-value {
            font-size: 20px;
            font-weight: 600;
            color: var(--el-color-primary);
          }
        }
      }

      .config-details {
        .agent-title {
          display: flex;
          align-items: center;
          gap: 8px;
          font-weight: 500;
        }

        .agent-services {
          .service-item {
            margin-bottom: 16px;
            padding: 16px;
            background: var(--el-fill-color-lighter);
            border-radius: 8px;

            .service-header {
              display: flex;
              align-items: center;
              justify-content: space-between;
              margin-bottom: 12px;

              h4 {
                margin: 0;
                font-size: 16px;
                color: var(--el-text-color-primary);
              }
            }

            .service-config {
              background: var(--el-bg-color-page);
              padding: 12px;
              border-radius: 6px;
              font-size: 12px;
              line-height: 1.4;
              color: var(--el-text-color-regular);
              max-height: 200px;
              overflow-y: auto;
              margin: 0;
            }
          }
        }
      }
    }
  }
}

// 响应式设计
@media (max-width: 768px) {
  .reset-manager {
    padding: 16px;

    .page-header {
      flex-direction: column;
      gap: 16px;

      .header-right {
        width: 100%;
        display: flex;
        justify-content: flex-end;
      }
    }

    .reset-cards {
      .reset-card {
        .reset-section {
          .reset-items {
            .reset-item {
              flex-direction: column;
              gap: 12px;

              .item-info {
                margin-bottom: 0;
              }
            }
          }
        }
      }
    }
  }
}
</style>
