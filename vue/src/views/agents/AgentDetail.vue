<template>
  <div class="page-container">
    <!-- Header -->
    <header class="page-header">
      <div class="header-left">
        <el-button 
          :icon="ArrowLeft" 
          circle 
          plain 
          class="back-btn"
          @click="$router.back()"
        />
        <div class="header-content">
          <h1 class="page-title">
            {{ agentId }}
          </h1>
          <p class="page-subtitle">
            Agent Context Detail
          </p>
        </div>
      </div>
      <div class="header-actions">
        <el-button 
          :icon="Plus" 
          type="primary"
          color="#000"
          class="action-btn"
          @click="openServiceDialog"
        >
          Add Service
        </el-button>
        <el-button 
          :icon="Refresh" 
          :loading="loading"
          circle
          plain
          class="action-btn"
          @click="refreshData"
        />
      </div>
    </header>

    <!-- KPI Metrics -->
    <div class="kpi-grid">
      <StatCard
        title="Services"
        :value="agentStats.services || 0"
        unit="nodes"
        :icon="Connection"
        class="kpi-card"
      />
      
      <StatCard
        title="Tools"
        :value="agentStats.tools || 0"
        unit="fns"
        :icon="Tools"
        class="kpi-card"
      />
      
      <StatCard
        title="Health"
        :value="agentStats.healthy_services || 0"
        unit="active"
        :icon="FirstAidKit"
        :class="['kpi-card', isHealthy ? 'status-active' : 'status-issue']"
      />
      
      <StatCard
        title="Executions"
        :value="agentStats.total_tool_executions || 0"
        unit="calls"
        :icon="VideoPlay"
        class="kpi-card"
      />
    </div>

    <!-- Main Content Layout -->
    <div class="main-layout">
      <!-- Left Column: Services & Tools -->
      <div class="content-column left-col">
        
        <!-- Services Panel -->
        <section class="panel-section">
          <div class="panel-header">
            <h3 class="panel-title">
              Active Services
            </h3>
            <div class="panel-controls">
               <span class="count-badge">{{ services.length }}</span>
            </div>
          </div>
          
          <div class="panel-body table-container">
            <el-table
              :data="services"
              class="atom-table"
              :show-header="true"
              size="small"
              empty-text="No services configured"
            >
              <el-table-column
                label="SERVICE"
                min-width="200"
              >
                <template #default="{ row }">
                  <div class="service-name-cell">
                    <div
                      class="status-indicator"
                      :class="getServiceStatusClass(row)"
                    />
                    <div class="name-wrapper">
                      <span class="primary-text">{{ row.name }}</span>
                      <div class="meta-row">
                        <el-tag size="small" effect="plain" class="type-tag">{{ row.command ? 'LOCAL' : 'REMOTE' }}</el-tag>
                        <span v-if="!row.is_active" class="config-badge">CONFIG ONLY</span>
                      </div>
                    </div>
                  </div>
                </template>
              </el-table-column>
              
              <el-table-column
                label="CONNECTION"
                min-width="220"
              >
                <template #default="{ row }">
                  <div class="connection-info">
                    <span v-if="row.url" class="mono-text url" :title="row.url">{{ row.url }}</span>
                    <span v-else-if="row.command" class="mono-text command" :title="row.command + ' ' + (row.args || []).join(' ')">
                      $ {{ row.command }}
                    </span>
                    <span class="transport-text">{{ row.transport || (row.command ? 'stdio' : 'http') }}</span>
                  </div>
                </template>
              </el-table-column>

              <el-table-column
                label="TOOLS"
                width="80"
                align="center"
              >
                <template #default="{ row }">
                  <span class="mono-number">{{ row.tool_count || 0 }}</span>
                </template>
              </el-table-column>
              
              <el-table-column
                label="ACTIONS"
                width="160"
                align="right"
              >
                <template #default="{ row }">
                  <div class="actions-group">
                    <el-button link size="small" @click="editService(row)">Edit</el-button>
                    <el-button link size="small" @click="restartService(row)">Restart</el-button>
                    <el-button link size="small" type="danger" @click="deleteService(row)">Del</el-button>
                  </div>
                </template>
              </el-table-column>
            </el-table>
          </div>
        </section>

        <!-- Tools Panel -->
        <section class="panel-section">
          <div class="panel-header">
            <h3 class="panel-title">
              Available Tools
            </h3>
            <span class="count-badge">{{ tools.length }}</span>
          </div>
          <div class="panel-body table-container">
            <el-table
              :data="tools"
              class="atom-table"
              :show-header="true"
              size="small"
              empty-text="No tools available"
            >
              <el-table-column
                label="TOOL NAME"
                min-width="180"
              >
                <template #default="{ row }">
                  <div class="tool-cell">
                    <span class="primary-text">{{ row.name }}</span>
                  </div>
                </template>
              </el-table-column>
              
              <el-table-column
                label="SOURCE SERVICE"
                width="160"
              >
                <template #default="{ row }">
                  <span class="secondary-text">{{ row.service_name }}</span>
                </template>
              </el-table-column>
              
              <el-table-column
                label="DESCRIPTION"
                min-width="200"
              >
                <template #default="{ row }">
                  <span class="secondary-text truncate" :title="row.description">{{ row.description || '-' }}</span>
                </template>
              </el-table-column>
              
              <el-table-column
                width="80"
                align="right"
              >
                <template #default="{ row }">
                   <el-button link size="small" @click="executeTool(row)">Run</el-button>
                </template>
              </el-table-column>
            </el-table>
          </div>
        </section>
      </div>

      <!-- Right Column: Info & Config -->
      <div class="content-column right-col">
        <!-- Agent Info -->
        <section class="panel-section">
          <div class="panel-header">
            <h3 class="panel-title">
              Context Info
            </h3>
          </div>
          <div class="info-list">
            <div class="info-item">
              <span>Agent ID</span>
              <span class="mono-val">{{ agentId }}</span>
            </div>
            <div class="info-item">
              <span>Status</span>
              <span 
                class="status-badge"
                :class="isHealthy ? 'active' : 'warn'"
              >
                {{ isHealthy ? 'Healthy' : 'Issues' }}
              </span>
            </div>
             <div class="info-item">
              <span>Orchestrator</span>
              <span class="mono-val">{{ agentStats.orchestrator_status || 'Active' }}</span>
            </div>
          </div>
        </section>

        <!-- Quick Actions -->
        <section class="panel-section">
           <div class="panel-header">
            <h3 class="panel-title">
              Actions
            </h3>
          </div>
          <div class="actions-grid">
             <button class="action-card-btn" @click="refreshData">
               <el-icon><Refresh /></el-icon>
               <span>Refresh Context</span>
             </button>
             <button class="action-card-btn" @click="$router.push('/for_store/tool_records')">
               <el-icon><notebook /></el-icon>
               <span>View Logs</span>
             </button>
          </div>
        </section>
      </div>
    </div>

    <!-- Edit Dialog (Preserved functionality, updated style) -->
    <el-dialog
      v-model="editDialogVisible"
      :title="`Edit Service: ${editingService?.name}`"
      width="600px"
      class="atom-dialog"
      :close-on-click-modal="false"
    >
      <div v-if="editingService" class="edit-content">
        <!-- JSON Edit Mode only for atom style simplicity -->
        <div class="json-editor-wrapper">
             <div class="editor-toolbar">
                <span class="label">Configuration (JSON)</span>
                <div class="tools">
                   <el-button size="small" link @click="formatEditJson">Format</el-button>
                   <el-button size="small" link @click="validateEditJson">Validate</el-button>
                </div>
             </div>
             <el-input
              v-model="editJsonContent"
              type="textarea"
              :rows="15"
              class="code-input"
              spellcheck="false"
            />
        </div>
      </div>
      <template #footer>
        <div class="dialog-footer">
          <el-button @click="editDialogVisible = false">Cancel</el-button>
          <el-button type="primary" :loading="editSaving" color="#000" @click="saveServiceEdit">
            Save Changes
          </el-button>
        </div>
      </template>
    </el-dialog>

    <el-dialog
      v-model="serviceDialogVisible"
      width="1000px"
      class="agent-service-dialog"
      destroy-on-close
      :close-on-click-modal="false"
      title="Add Service"
    >
      <ServiceForm
        :default-agent-id="agentId"
        compact
        @success="handleServiceAdded"
      />
    </el-dialog>

  </div>
</template>

<script setup>
import { ref, onMounted, computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import { 
  ArrowLeft, Plus, Refresh, Connection, Tools, 
  VideoPlay, Notebook, FirstAidKit 
} from '@element-plus/icons-vue'
import { useAgentsStore } from '@/stores/agents'
import StatCard from '@/components/common/StatCard.vue'
import ServiceForm from '@/components/agents/ServiceForm.vue'

const route = useRoute()
const router = useRouter()
const agentsStore = useAgentsStore()

// Data
const loading = ref(false)
const services = ref([])
const tools = ref([])
const agentStats = ref({})
const serviceDialogVisible = ref(false)

// Edit State
const editDialogVisible = ref(false)
const editingService = ref(null)
const editJsonContent = ref('')
const editSaving = ref(false)

// Computed
const agentId = computed(() => route.params.id)
const isHealthy = computed(() => {
  return (agentStats.value.healthy_services || 0) === (agentStats.value.services || 0) && (agentStats.value.services > 0)
})

// Methods
const refreshData = async () => {
  loading.value = true
  try {
    const [svcData, toolsData] = await Promise.all([
      agentsStore.getAgentServices(agentId.value),
      agentsStore.getAgentTools(agentId.value)
    ])
    const statsData = agentsStore.buildAgentStats(svcData, toolsData)
    services.value = svcData || []
    tools.value = toolsData || []
    agentStats.value = statsData || {}
    ElMessage.success('Context refreshed')
  } catch (error) {
    ElMessage.error('Failed to refresh: ' + error.message)
  } finally {
    loading.value = false
  }
}

const getServiceStatusClass = (row) => {
  if (row.is_active) return 'is-active'
  if (row.status === 'error') return 'is-error'
  return 'is-inactive'
}

// Actions
const openServiceDialog = () => {
  serviceDialogVisible.value = true
}

const handleServiceAdded = async () => {
  serviceDialogVisible.value = false
  await refreshData()
}

const executeTool = (tool) => {
  router.push({
    path: '/for_store/call_tool',
    query: {
      agentId: agentId.value,
      toolName: tool.name,
      serviceName: tool.service_name
    }
  })
}

const restartService = async (service) => {
  try {
    const identifier = service.client_id || service.name
    await agentsStore.restartService(agentId.value, identifier)
    ElMessage.success(`Service ${service.name} restarting...`)
    setTimeout(refreshData, 1000)
  } catch (error) {
    ElMessage.error(`Restart failed: ${error.message}`)
  }
}

const deleteService = async (service) => {
  try {
    await ElMessageBox.confirm(
      `Remove service "${service.name}" from this agent?`,
      'Confirm Deletion',
      { type: 'warning', confirmButtonText: 'Delete', cancelButtonText: 'Cancel' }
    )
    const { api } = await import('@/api')
    const identifier = service.client_id || service.name
    const res = await api.agent.deleteConfig(agentId.value, identifier)
    
    if (res.data.success) {
      ElMessage.success('Service removed')
      refreshData()
    } else {
      throw new Error(res.data.message)
    }
  } catch (e) {
    if (e !== 'cancel') ElMessage.error(e.message)
  }
}

// Edit Logic
const editService = async (service) => {
  try {
    editingService.value = service
    const { api } = await import('@/api')
    const res = await api.agent.showConfig(agentId.value)
    
    if (res.data.success) {
      const fullConfig = res.data.data.services?.[service.name]?.config || {}
      // Fallback if config not found
      if (!Object.keys(fullConfig).length) {
         if (service.url) fullConfig.url = service.url
         if (service.command) fullConfig.command = service.command
      }
      
      editJsonContent.value = JSON.stringify({ [service.name]: fullConfig }, null, 2)
      editDialogVisible.value = true
    }
  } catch (e) {
    ElMessage.error('Failed to load config')
  }
}

const formatEditJson = () => {
  try {
    const p = JSON.parse(editJsonContent.value)
    editJsonContent.value = JSON.stringify(p, null, 2)
  } catch (e) { ElMessage.error('Invalid JSON') }
}

const validateEditJson = () => {
  try {
    JSON.parse(editJsonContent.value)
    ElMessage.success('Valid JSON')
  } catch (e) { ElMessage.error('Invalid JSON') }
}

const saveServiceEdit = async () => {
  try {
    editSaving.value = true
    const parsed = JSON.parse(editJsonContent.value)
    const serviceName = editingService.value.name
    const config = parsed[serviceName] || parsed
    
    const { api } = await import('@/api')
    const identifier = editingService.value.client_id || serviceName
    
    const res = await api.agent.updateConfig(agentId.value, identifier, config)
    if (res.data.success) {
      ElMessage.success('Configuration updated')
      editDialogVisible.value = false
      refreshData()
    } else {
      throw new Error(res.data.message)
    }
  } catch (e) {
    ElMessage.error(e.message)
  } finally {
    editSaving.value = false
  }
}

onMounted(refreshData)
</script>

<style lang="scss" scoped>
.page-container {
  max-width: 1440px;
  margin: 0 auto;
  padding: 20px;
  width: 100%;
}

// Header
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 24px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--border-color);
  
  .header-left {
    display: flex;
    align-items: center;
    gap: 16px;
    
    .back-btn {
      border-color: var(--border-color);
      color: var(--text-secondary);
      &:hover { color: var(--text-primary); border-color: var(--text-secondary); }
    }
  }
}

.page-title {
  font-size: 20px;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 2px;
}

.page-subtitle {
  font-size: 13px;
  color: var(--text-secondary);
  font-family: var(--font-mono);
}

.header-actions {
  display: flex;
  gap: 12px;
}

.action-btn {
  font-weight: 500;
  border-radius: 6px;
}

// KPI Grid
.kpi-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 16px;
  margin-bottom: 24px;

  @media (max-width: 1024px) { grid-template-columns: repeat(2, 1fr); }
  @media (max-width: 640px) { grid-template-columns: 1fr; }
}

.kpi-card {
  height: 100%;
  &.status-active { border-left: 3px solid var(--color-success) !important; }
  &.status-issue { border-left: 3px solid var(--color-danger) !important; }
}

// Layout
.main-layout {
  display: grid;
  grid-template-columns: 2.5fr 1fr;
  gap: 20px;
  
  @media (max-width: 1024px) { grid-template-columns: 1fr; }
}

.content-column {
  display: flex;
  flex-direction: column;
  gap: 24px;
}

// Panels
.panel-section {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  
  .panel-title {
    font-size: 13px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-secondary);
  }
  
  .count-badge {
    background: var(--bg-hover);
    padding: 2px 8px;
    border-radius: 10px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
  }
}

.panel-body {
  background: var(--bg-surface);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

// Table Styles
.table-container {
  overflow-x: auto;
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
    padding: 10px 16px;
  }

  td.el-table__cell {
    border-bottom: 1px solid var(--border-color) !important;
    padding: 12px 16px;
    font-size: 13px;
  }
  
  .el-table__inner-wrapper::before { display: none; }
}

// Service Cell
.service-name-cell {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  
  .status-indicator {
    margin-top: 6px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
    &.is-active { background: var(--color-success); box-shadow: 0 0 0 2px rgba(16, 185, 129, 0.1); }
    &.is-error { background: var(--color-danger); }
    &.is-inactive { background: var(--text-disabled); }
  }
  
  .name-wrapper {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  
  .meta-row {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  
  .type-tag {
    height: 18px;
    font-size: 10px;
    padding: 0 4px;
    border-radius: 3px;
    background: var(--bg-hover);
    color: var(--text-secondary);
    border: none;
  }
  
  .config-badge {
    font-size: 10px;
    color: var(--color-warning);
    font-weight: 500;
  }
}

// Connection Cell
.connection-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  
  .mono-text {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-primary);
    
    &.url { color: var(--color-accent); text-decoration: underline; cursor: pointer; }
    &.command { color: var(--text-primary); background: var(--bg-hover); padding: 2px 4px; border-radius: 3px; display: inline-block; width: fit-content;}
  }
  
  .transport-text {
    font-size: 11px;
    color: var(--text-secondary);
  }
}

// Common Text
.primary-text { font-weight: 500; color: var(--text-primary); }
.secondary-text { font-size: 12px; color: var(--text-secondary); }
.mono-number { font-family: var(--font-mono); font-size: 12px; color: var(--text-secondary); }
.truncate { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; display: block; max-width: 100%; }

// Actions
.actions-group {
  .el-button { padding: 0 4px; font-size: 12px; }
}

// Info List (Right Col)
.info-list {
  background: var(--bg-surface);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: 0 16px;
  
  .info-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 14px 0;
    border-bottom: 1px solid var(--border-color);
    font-size: 13px;
    color: var(--text-secondary);
    
    &:last-child { border-bottom: none; }
    
    .mono-val { font-family: var(--font-mono); color: var(--text-primary); }
    
    .status-badge {
      padding: 2px 8px;
      border-radius: 10px;
      font-size: 11px;
      font-weight: 500;
      &.active { background: rgba(16, 185, 129, 0.1); color: var(--color-success); }
      &.warn { background: rgba(245, 158, 11, 0.1); color: var(--color-warning); }
    }
  }
}

// Actions Grid
.actions-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: 10px;
  
  .action-card-btn {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 16px;
    background: var(--bg-surface);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-lg);
    cursor: pointer;
    transition: all 0.2s;
    
    &:hover { border-color: var(--text-secondary); background: var(--bg-hover); }
    
    .el-icon { font-size: 18px; color: var(--text-secondary); }
    span { font-size: 13px; font-weight: 500; color: var(--text-primary); }
  }
}

// Dialog
.editor-toolbar {
  display: flex;
  justify-content: space-between;
  margin-bottom: 8px;
  .label { font-size: 12px; font-weight: 600; color: var(--text-secondary); }
}

:deep(.code-input) {
  textarea {
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.5;
    background: var(--bg-hover);
    color: var(--text-primary);
    border: 1px solid var(--border-color);
    padding: 12px;
    
    &:focus { border-color: var(--text-secondary); box-shadow: none; }
  }
}
</style>
