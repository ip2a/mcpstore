<template>
  <div class="page-shell service-list-container">
    <!-- Header -->
    <header class="page-header">
      <div class="header-content">
        <h1 class="page-title">
          Service Registry
        </h1>
        <p class="page-subtitle">
          Manage and monitor MCP service instances
        </p>
      </div>
      <div class="header-actions">
        <el-button 
          :icon="Plus" 
          type="primary"
          color="#000" 
          class="create-btn"
          @click="$router.push('/for_store/add_service')"
        >
          Add Service
        </el-button>
        <el-dropdown
          trigger="click"
          @command="handleQuickAction"
        >
          <el-button
            :icon="Tools"
            circle
            plain
            class="action-icon-btn"
          />
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item command="reset-config">
                Reset Store Config
              </el-dropdown-item>
              <el-dropdown-item command="reset-manager">
                Reset Manager
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
      </div>
    </header>

    <!-- KPI Grid -->
    <div class="kpi-grid">
      <StatCard
        title="Total Services"
        :value="servicesData?.total_services || 0"
        unit="active"
        :icon="Connection"
        class="kpi-card"
      />
      <StatCard
        title="Healthy"
        :value="healthyServicesCount"
        unit="ok"
        :icon="Check"
        class="kpi-card"
      />
      <StatCard
        title="Issues"
        :value="(servicesData?.total_services || 0) - healthyServicesCount"
        unit="warn"
        :icon="Warning"
        class="kpi-card"
      />
      <StatCard
        title="Total Tools"
        :value="systemStore.tools.length"
        unit="fns"
        :icon="Search"
        class="kpi-card"
      />
    </div>

    <!-- Main Content -->
    <section class="panel-section">
      <!-- Controls -->
      <div class="panel-header">
        <h3 class="panel-title">
          Services List
        </h3>
        <div class="panel-controls">
          <!-- Batch Actions -->
          <div
            v-if="selectedServices.length > 0"
            class="batch-actions"
          >
            <span class="selection-count">{{ selectedServices.length }} selected</span>
            <el-button
              link
              size="small"
              @click="handleBatchRestart"
            >
              Restart
            </el-button>
            <el-button
              link
              size="small"
              type="danger"
              @click="handleBatchDelete"
            >
              Delete
            </el-button>
            <div class="divider" />
          </div>

          <!-- Filters -->
          <div class="search-wrapper">
            <el-icon class="search-icon">
              <Search />
            </el-icon>
            <input 
              v-model="searchQuery" 
              class="atom-input search-input" 
              placeholder="Search services..."
            >
          </div>
           
          <select
            v-model="typeFilter"
            class="atom-input filter-select"
          >
            <option value="">
              All Types
            </option>
            <option value="stdio">
              Stdio
            </option>
            <option value="sse">
              SSE
            </option>
            <option value="streamable_http">
              HTTP (Stream)
            </option>
          </select>

          <select
            v-model="statusFilter"
            class="atom-input filter-select"
          >
            <option value="">
              All Status
            </option>
            <option value="ready">
              Ready
            </option>
            <option value="healthy">
              Healthy
            </option>
            <option value="degraded">
              Degraded
            </option>
            <option value="half_open">
              Half Open
            </option>
            <option value="circuit_open">
              Circuit Open
            </option>
            <option value="disconnected">
              Disconnected
            </option>
          </select>

          <el-button 
            :icon="Refresh" 
            :loading="refreshLoading"
            circle 
            plain 
            class="refresh-btn"
            @click="refreshServices"
          />
        </div>
      </div>

      <!-- Table -->
      <div class="panel-body table-container">
        <el-table 
          v-loading="loading"
          :data="filteredServices" 
          class="atom-table" 
          :show-header="true" 
          size="small"
          @selection-change="handleSelectionChange"
        >
          <el-table-column
            type="selection"
            width="40"
          />
          
          <el-table-column
            prop="name"
            label="SERVICE"
            min-width="180"
          >
            <template #default="{ row }">
              <div class="service-identity">
                <div
                  class="status-dot"
                  :class="getStatusClass(row.status)"
                />
                <div class="name-col">
                  <span class="primary-text font-medium">{{ row.name }}</span>
                  <span class="secondary-text mono-text">{{ row.type }}</span>
                </div>
              </div>
            </template>
          </el-table-column>
          
          <el-table-column
            label="CONFIG"
            min-width="240"
          >
            <template #default="{ row }">
              <div class="config-detail">
                <template v-if="row.url">
                  <span
                    class="secondary-text truncate"
                    :title="row.url"
                  >{{ row.url }}</span>
                </template>
                <template v-else>
                  <span
                    class="secondary-text font-mono truncate"
                    :title="`${row.command} ${(row.args||[]).join(' ')}`"
                  >
                    $ {{ row.command }}
                  </span>
                </template>
              </div>
            </template>
          </el-table-column>

          <el-table-column
            prop="tools_count"
            label="TOOLS"
            width="100"
            align="right"
          >
            <template #default="{ row }">
              <span 
                class="badge-number" 
                :class="{ 'has-tools': row.tools_count > 0 }"
                @click.stop="viewServiceTools(row)"
              >
                {{ row.tools_count || 0 }}
              </span>
            </template>
          </el-table-column>

          <el-table-column
            label="STATUS"
            width="120"
          >
            <template #default="{ row }">
              <span
                class="status-text"
                :class="getStatusClass(row.status)"
              >
                {{ getStatusLabel(row.status) }}
              </span>
            </template>
          </el-table-column>

          <el-table-column
            label="HEALTH"
            min-width="230"
          >
            <template #default="{ row }">
              <div class="metric-line">
                <span class="metric-label">错误率</span>
                <span
                  :class="['metric-value', metricTone(row.window_error_rate)]"
                >{{ formatErrorRate(row.window_error_rate) }}</span>
              </div>
              <div class="metric-line">
                <span class="metric-label">P95 / P99</span>
                <span class="metric-value">
                  {{ formatLatency(row.latency_p95) }} / {{ formatLatency(row.latency_p99) }}
                </span>
              </div>
              <div class="metric-line">
                <span class="metric-label">样本</span>
                <span class="metric-value">
                  {{ formatSampleSize(row.sample_size) }}
                </span>
              </div>
              <div class="metric-line timing">
                <span class="metric-label">重试</span>
                <span class="metric-value">
                  {{ formatRemainingLabel(row.retry_in, row.next_retry_time) }}
                </span>
              </div>
            </template>
          </el-table-column>

          <el-table-column
            label=""
            width="180"
            align="right"
          >
            <template #default="{ row }">
              <div class="row-actions">
                <button
                  class="text-btn"
                  @click="viewServiceDetails(row)"
                >
                  Detail
                </button>
                <button
                  class="text-btn"
                  @click="editService(row)"
                >
                  Config
                </button>
                <button 
                  class="text-btn" 
                  :disabled="row.restarting"
                  @click="restartService(row)"
                >
                  {{ row.restarting ? '...' : 'Restart' }}
                </button>
              </div>
            </template>
          </el-table-column>
          
          <template #empty>
            <div class="empty-state">
              <span class="empty-text">No services found</span>
            </div>
          </template>
        </el-table>
      </div>
    </section>

    <!-- Hidden Batch Operations Logic (Preserved functionality) -->
    <BatchOperations
      v-show="false"
      ref="batchOperationsRef"
      :items="systemStore.services"
      item-key="name"
      item-name="name"
      @batch-edit="handleBatchEdit"
      @batch-delete="handleBatchDelete"
      @selection-change="handleBatchSelectionChange"
    />

    <!-- Edit Dialog (Re-styled) -->
    <el-dialog
      v-model="editDialogVisible"
      :title="editingService?.name"
      width="600px"
      class="atom-dialog"
      align-center
      :show-close="false"
    >
      <div
        v-if="editingService"
        class="dialog-content"
      >
        <div class="edit-mode-tabs">
          <span 
            :class="['tab-item', { active: editMode === 'fields' }]"
            @click="editMode = 'fields'"
          >Fields</span>
          <span 
            :class="['tab-item', { active: editMode === 'json' }]"
            @click="editMode = 'json'"
          >JSON</span>
        </div>

        <!-- Fields Mode -->
        <div
          v-if="editMode === 'fields'"
          class="form-container"
        >
          <!-- Common Fields -->
          <div
            v-if="isRemoteService"
            class="form-group"
          >
            <label>URL</label>
            <input
              v-model="editForm.url"
              class="atom-input full"
              placeholder="https://..."
            >
          </div>

          <template v-else>
            <div class="form-group">
              <label>Command</label>
              <input
                v-model="editForm.command"
                class="atom-input full"
                placeholder="e.g. npx"
              >
            </div>
            <div class="form-group">
              <label>Args</label>
              <input
                v-model="editFormArgsString"
                class="atom-input full"
                placeholder="Arguments separated by spaces"
              >
            </div>
          </template>

          <div class="form-group">
            <label>Environment Variables (KEY=VALUE per line)</label>
            <textarea
              v-model="editFormEnvString"
              class="atom-input full"
              rows="4"
            />
          </div>
        </div>

        <!-- JSON Mode -->
        <div
          v-else
          class="json-container"
        >
          <textarea
            v-model="editJsonContent"
            class="code-editor"
            rows="12"
          />
        </div>
      </div>

      <template #footer>
        <div class="dialog-footer">
          <div
            v-if="editMode === 'json'"
            class="left-actions"
          >
            <button
              class="text-btn"
              @click="formatEditJson"
            >
              Format
            </button>
          </div>
          <div class="right-actions">
            <el-button
              text
              @click="editDialogVisible = false"
            >
              Cancel
            </el-button>
            <el-button
              type="primary"
              color="#000"
              :loading="editSaving"
              @click="saveServiceEdit"
            >
              Save Changes
            </el-button>
          </div>
        </div>
      </template>
    </el-dialog>

    <BatchUpdateDialog
      v-model="batchUpdateDialogVisible"
      :services="selectedServices"
      @updated="handleBatchUpdateSuccess"
    />
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import { ElMessage, ElMessageBox } from 'element-plus'
import BatchOperations from '@/components/BatchOperations.vue'
import BatchUpdateDialog from './BatchUpdateDialog.vue'
import StatCard from '@/components/common/StatCard.vue'
import { 
  Plus, Refresh, Search, Tools, Connection, Warning, Check 
} from '@element-plus/icons-vue'
import { getStatusMeta } from '@/utils/serviceStatus'
import { formatErrorRate, formatLatency, formatSampleSize, errorRateLevel, formatRemaining } from '@/utils/healthMetrics'

const router = useRouter()
const route = useRoute()
const systemStore = useSystemStore()

// State
const loading = ref(false)
const refreshLoading = ref(false)
const searchQuery = ref('')
const statusFilter = ref('')
const typeFilter = ref('')
const servicesData = ref(null)
const selectedServices = ref([])
const now = ref(Date.now())

// Edit State
const editDialogVisible = ref(false)
const editingService = ref(null)
const editMode = ref('fields')
const editForm = ref({})
const editJsonContent = ref('')
const editSaving = ref(false)
const editFormArgsString = ref('')
const editFormEnvString = ref('')
const batchUpdateDialogVisible = ref(false)
const batchOperationsRef = ref(null)
let heartbeatTimer = null

// Computed
const isRemoteService = computed(() => editForm.value.url && !editForm.value.command)

const filteredServices = computed(() => {
  let services = systemStore.services
  if (searchQuery.value) {
    const q = searchQuery.value.toLowerCase()
    services = services.filter(s => 
      s.name.toLowerCase().includes(q) || 
      (s.url && s.url.toLowerCase().includes(q)) ||
      (s.command && s.command.toLowerCase().includes(q))
    )
  }
  if (typeFilter.value) {
    services = services.filter(s => s.type === typeFilter.value)
  }
  if (statusFilter.value) {
    services = services.filter(s => s.status === statusFilter.value)
  }
  return services
})

const healthyServicesCount = computed(() => {
  return systemStore.services.filter(s => s.status === 'healthy').length
})

// 状态与指标辅助
const getStatusClass = (status) => getStatusMeta(status).className || 'is-unknown'
const getStatusLabel = (status) => getStatusMeta(status).text
const metricTone = (rate) => {
  const level = errorRateLevel(rate)
  if (level === 'danger') return 'is-danger'
  if (level === 'warn') return 'is-warn'
  return ''
}
const formatRemainingLabel = (value, deadline) => formatRemaining(value, deadline, now.value)
const mergeHealthMetrics = (services, healthList) => {
  const map = {}
  const list = Array.isArray(healthList?.services) ? healthList.services : Array.isArray(healthList) ? healthList : []
  list.forEach(info => {
    const key = info?.name || info?.service_name
    if (key) map[key] = info
  })

  return (services || []).map(service => {
    const key = service.name || service.service_name
    const health = map[key]
    if (!health) return service
    return {
      ...service,
      status: health.status || service.status,
      window_error_rate: health.window_error_rate ?? service.window_error_rate,
      latency_p95: health.latency_p95 ?? service.latency_p95,
      latency_p99: health.latency_p99 ?? service.latency_p99,
      sample_size: health.sample_size ?? service.sample_size,
      retry_in: health.retry_in ?? service.retry_in,
      hard_timeout_in: health.hard_timeout_in ?? service.hard_timeout_in,
      lease_remaining: health.lease_remaining ?? service.lease_remaining,
      next_retry_time: health.next_retry_time ?? service.next_retry_time,
      hard_deadline: health.hard_deadline ?? service.hard_deadline,
      lease_deadline: health.lease_deadline ?? service.lease_deadline
    }
  })
}

const refreshServices = async () => {
  refreshLoading.value = true
  try {
    const { api } = await import('@/api')
    const servicesArr = await api.store.listServices()
    let healthData = []
    try {
      healthData = await api.store.checkServices()
    } catch (err) {
      // 健康检查失败不阻塞列表刷新，保持静默退化
      console.warn('健康状态获取失败:', err?.message || err)
    }
    const mergedServices = mergeHealthMetrics(servicesArr, healthData)
    servicesData.value = { services: mergedServices, total_services: mergedServices.length }
    systemStore.services = mergedServices
    systemStore.updateStats()
    ElMessage.success('Refreshed')
  } catch (error) {
    ElMessage.error('Failed to refresh')
  } finally {
    refreshLoading.value = false
  }
}

const handleSelectionChange = (selection) => {
  selectedServices.value = selection
}

const viewServiceDetails = (row) => {
  router.push({ 
    path: `/for_store/service_info/${row.name}`,
    query: route.query.agent ? { agent: route.query.agent } : {}
  })
}

const viewServiceTools = (row) => {
  router.push({ path: '/for_store/list_tools', query: { service: row.name } })
}

const restartService = async (service) => {
  try {
    service.restarting = true
    await systemStore.restartService(service.name)
    ElMessage.success(`Restarted ${service.name}`)
  } catch (error) {
    ElMessage.error(`Failed to restart ${service.name}`)
  } finally {
    service.restarting = false
  }
}

const handleQuickAction = async (cmd) => {
  if (cmd === 'reset-config') {
    try {
      await ElMessageBox.confirm('Reset all store config?', 'Confirm', { type: 'warning' })
      const { api } = await import('@/api')
      await api.store.resetConfig()
      ElMessage.success('Reset successful')
      refreshServices()
    } catch (e) { /* cancelled */ }
  } else if (cmd === 'reset-manager') {
    router.push('/system/reset')
  }
}

// Edit Logic
const editService = async (service) => {
  editingService.value = service
  editMode.value = 'fields'
  
  // Use existing data or fetch fresh? Fetching fresh is safer as per original code
  try {
     const { api } = await import('@/api')
     const res = await api.store.getConfig('global')
     let config = null
     
     if (res.data.success && res.data.data.services) {
       config = res.data.data.services[service.name]?.config
     }

     // Setup Form
     if (config) {
       editForm.value = { ...config }
       editFormArgsString.value = Array.isArray(config.args) ? config.args.join(' ') : ''
       editFormEnvString.value = config.env ? Object.entries(config.env).map(([k,v])=>`${k}=${v}`).join('\n') : ''
       editJsonContent.value = JSON.stringify({ [service.name]: config }, null, 2)
     } else {
       // Fallback
       editForm.value = { command: '', args: [], env: {} }
       editFormArgsString.value = ''
       editJsonContent.value = '{}'
     }
     editDialogVisible.value = true
  } catch (e) {
    ElMessage.error('Failed to load config')
  }
}

const saveServiceEdit = async () => {
  try {
    editSaving.value = true
    const { api } = await import('@/api')
    let config = { ...editForm.value }
    
    if (editMode.value === 'fields') {
       if (!isRemoteService.value) {
          config.args = editFormArgsString.value.trim().split(/\s+/).filter(Boolean)
       }
       config.env = {}
       editFormEnvString.value.split('\n').forEach(line => {
         const [k, ...v] = line.split('=')
         if(k && v) config.env[k.trim()] = v.join('=').trim()
       })
    } else {
       try {
         const parsed = JSON.parse(editJsonContent.value)
         config = parsed[editingService.value.name] || parsed
       } catch {
         ElMessage.error('Invalid JSON')
         return
       }
    }

    const res = await api.store.updateConfig(editingService.value.name, config)
    if (res.data.success) {
      ElMessage.success('Saved')
      editDialogVisible.value = false
      refreshServices()
    } else {
      ElMessage.error(res.data.message || 'Error saving')
    }
  } catch (e) {
    ElMessage.error(e.message)
  } finally {
    editSaving.value = false
  }
}

const formatEditJson = () => {
  try {
    const p = JSON.parse(editJsonContent.value)
    editJsonContent.value = JSON.stringify(p, null, 2)
  } catch (e) {
    ElMessage.warning('Invalid JSON')
  }
}

// Batch Logic Stubs (Preserving original calls)
const handleBatchRestart = async () => {
   if (!selectedServices.value.length) return
   try {
     await ElMessageBox.confirm(`Restart ${selectedServices.value.length} services?`, 'Confirm')
     const { api } = await import('@/api')
     await api.store.batchRestartServices(selectedServices.value.map(s => s.name))
     ElMessage.success('Batch restart initiated')
     refreshServices()
   } catch (e) { /* cancelled */ }
}

const handleBatchDelete = async () => {
   if (!selectedServices.value.length) return
   try {
     await ElMessageBox.confirm(`Delete ${selectedServices.value.length} services?`, 'Warning', { type: 'warning' })
     const { api } = await import('@/api')
     await api.store.batchDeleteServices(selectedServices.value.map(s => s.name))
     ElMessage.success('Services deleted')
     refreshServices()
   } catch (e) { /* cancelled */ }
}

const handleBatchUpdateSuccess = () => refreshServices()
const handleBatchSelectionChange = (s) => selectedServices.value = s
const handleBatchEdit = () => {} // Handled via ref

onMounted(async () => {
  heartbeatTimer = setInterval(() => {
    now.value = Date.now()
  }, 1000)
  await refreshServices()
  // Ensure tools are loaded for count stats
  if (systemStore.tools.length === 0) {
    systemStore.fetchTools()
  }
})

onUnmounted(() => {
  if (heartbeatTimer) {
    clearInterval(heartbeatTimer)
    heartbeatTimer = null
  }
})
</script>

<style lang="scss" scoped>
.service-list-container {
  width: 100%;
}

// Header
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  margin-bottom: 24px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--border-color);
}

.page-title {
  font-size: 20px;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 4px;
}

.page-subtitle {
  font-size: 13px;
  color: var(--text-secondary);
}

.header-actions {
  display: flex;
  gap: 8px;
}

.create-btn {
  font-weight: 500;
  border-radius: 6px;
}

.action-icon-btn {
  border: 1px solid var(--border-color);
  &:hover { background: var(--bg-hover); border-color: var(--text-secondary); }
}

// KPI Grid
.kpi-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 16px;
  margin-bottom: 24px;
  
  @media (max-width: 900px) { grid-template-columns: repeat(2, 1fr); }
  @media (max-width: 600px) { grid-template-columns: 1fr; }
}

.kpi-card {
  height: 100%;
}

// Panel & Controls
.panel-section {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 4px;
}

.panel-title {
  font-size: 13px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-secondary);
}

.panel-controls {
  display: flex;
  gap: 8px;
  align-items: center;
}

// Batch Actions
.batch-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-right: 12px;
  
  .selection-count {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-primary);
    background: var(--bg-hover);
    padding: 2px 8px;
    border-radius: 4px;
  }
  
  .divider {
    width: 1px;
    height: 16px;
    background: var(--border-color);
    margin: 0 8px;
  }
}

// Atomic Inputs
.search-wrapper {
  position: relative;
  .search-icon {
    position: absolute;
    left: 8px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-placeholder);
    font-size: 14px;
  }
  .search-input { padding-left: 28px; width: 200px; }
}

.atom-input {
  border: 1px solid var(--border-color);
  background: var(--bg-surface);
  padding: 6px 12px;
  border-radius: 6px;
  font-size: 13px;
  color: var(--text-primary);
  transition: border-color 0.2s;
  
  &:focus { outline: none; border-color: var(--text-secondary); }
  &::placeholder { color: var(--text-placeholder); }
  
  &.full { width: 100%; box-sizing: border-box; }
}

.filter-select {
  width: 130px;
  cursor: pointer;
}

.refresh-btn {
  border-color: var(--border-color);
  color: var(--text-secondary);
  &:hover { color: var(--text-primary); border-color: var(--text-secondary); background: transparent; }
}

// Table
.table-container {
  background: var(--bg-surface);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  overflow: hidden;
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
    text-transform: uppercase;
  }

  td.el-table__cell {
    border-bottom: 1px solid var(--border-color) !important;
    padding: 12px 16px;
  }
  
  .el-table__inner-wrapper::before { display: none; }
}

// Cell Content
.service-identity {
  display: flex;
  align-items: center;
  gap: 12px;
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
  
  &.is-ready { background-color: #0ea5e9; box-shadow: 0 0 0 2px rgba(14, 165, 233, 0.15); }
  &.is-healthy { background-color: var(--color-success); box-shadow: 0 0 0 2px rgba(16, 185, 129, 0.1); }
  &.is-init,
  &.is-startup { background-color: var(--color-accent); }
  &.is-degraded { background-color: var(--color-warning); }
  &.is-half-open { background-color: #f97316; box-shadow: 0 0 0 2px rgba(249, 115, 22, 0.12); }
  &.is-circuit-open { background-color: var(--color-danger); box-shadow: 0 0 0 2px rgba(239, 68, 68, 0.15); }
  &.is-disconnected { background-color: #6b7280; }
  &.is-unknown { background-color: var(--text-placeholder); }
}

.name-col {
  display: flex;
  flex-direction: column;
}

.primary-text {
  font-size: 13px;
  color: var(--text-primary);
  &.font-medium { font-weight: 500; }
}

.secondary-text {
  font-size: 12px;
  color: var(--text-secondary);
  &.mono-text { font-family: var(--font-mono); font-size: 11px; }
}

.truncate {
  display: block;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100%;
}

.badge-number {
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--text-placeholder);
  padding: 2px 6px;
  border-radius: 4px;
  cursor: default;
  
  &.has-tools {
    color: var(--text-primary);
    background: var(--bg-hover);
    cursor: pointer;
    &:hover { background: var(--bg-active); }
  }
}

.status-text {
  font-size: 12px;
  text-transform: none;
  
  &.is-ready { color: #0284c7; }
  &.is-healthy { color: var(--color-success); }
  &.is-degraded { color: var(--color-warning); }
  &.is-half-open { color: #c2410c; }
  &.is-circuit-open { color: var(--color-danger); }
  &.is-init,
  &.is-startup { color: var(--color-accent); }
  &.is-disconnected,
  &.is-unknown { color: var(--text-secondary); }
}

.metric-line {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-secondary);

  & + .metric-line { margin-top: 4px; }
}

.metric-label {
  color: var(--text-secondary);
}

.metric-value {
  font-family: var(--font-mono);
  color: var(--text-primary);

  &.is-warn { color: var(--color-warning); }
  &.is-danger { color: var(--color-danger); }
}

// Actions
.row-actions {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
}

.text-btn {
  background: none;
  border: none;
  cursor: pointer;
  font-size: 12px;
  font-weight: 500;
  color: var(--text-secondary);
  padding: 0;
  
  &:hover { color: var(--text-primary); text-decoration: underline; }
  &:disabled { opacity: 0.5; cursor: not-allowed; text-decoration: none; }
}

.empty-state {
  padding: 40px;
  text-align: center;
  color: var(--text-placeholder);
  font-size: 13px;
}

// Dialog
.dialog-content {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.edit-mode-tabs {
  display: flex;
  gap: 20px;
  border-bottom: 1px solid var(--border-color);
  margin-bottom: 8px;
  
  .tab-item {
    font-size: 13px;
    font-weight: 500;
    padding-bottom: 8px;
    color: var(--text-secondary);
    cursor: pointer;
    position: relative;
    
    &.active {
      color: var(--text-primary);
      &::after {
        content: '';
        position: absolute;
        bottom: -1px;
        left: 0;
        width: 100%;
        height: 2px;
        background: var(--text-primary);
      }
    }
  }
}

.form-container, .json-container {
  padding: 8px 0;
}

.form-group {
  margin-bottom: 16px;
  label {
    display: block;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    margin-bottom: 6px;
    text-transform: uppercase;
  }
}

.code-editor {
  width: 100%;
  font-family: var(--font-mono);
  font-size: 12px;
  padding: 12px;
  background: var(--bg-body);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  color: var(--text-primary);
  line-height: 1.5;
  resize: vertical;
  box-sizing: border-box;
  
  &:focus { outline: none; border-color: var(--text-secondary); }
}

.dialog-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  
  .right-actions {
    display: flex;
    gap: 8px;
    margin-left: auto;
  }
}
</style>
