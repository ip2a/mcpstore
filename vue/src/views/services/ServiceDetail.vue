<template>
  <div class="service-detail-container">
    <!-- Header -->
    <header class="page-header">
      <div class="header-content">
        <h1 class="page-title">
          Service Details
        </h1>
        <p class="page-subtitle">
          Inspect and manage service configuration
        </p>
      </div>
      <div class="header-actions">
        <el-button
          link
          class="back-link"
          @click="$router.back()"
        >
          <el-icon><ArrowLeft /></el-icon> Back
        </el-button>
      </div>
    </header>

    <!-- Loading State -->
    <div
      v-if="pageLoading"
      class="loading-state"
    >
      <el-icon class="is-loading">
        <Loading />
      </el-icon>
      <span>Loading service details...</span>
    </div>

    <!-- Main Content -->
    <div
      v-else-if="serviceData"
      class="main-layout"
    >
      <!-- Left Column: Info & Config -->
      <div class="panel-column left-col">
        <!-- Identity Card -->
        <section class="panel-section">
          <div class="panel-header">
            <h3 class="panel-title">
              Identity
            </h3>
            <div class="panel-controls">
              <StatusBadge
                v-if="serviceData.status"
                :status="serviceData.status"
                size="small"
              />
              <span
                v-else
                class="status-badge unknown"
              >
                UNKNOWN
              </span>
            </div>
          </div>
          <div class="panel-body info-card">
            <div class="info-row">
              <label>Name</label>
              <span class="value">{{ serviceData.name }}</span>
            </div>
            <div class="info-row">
              <label>Type</label>
              <span class="tag">{{ serviceData.command ? 'Local (Stdio)' : 'Remote' }}</span>
            </div>
            <div class="info-row">
              <label>Client ID</label>
              <span class="value mono">{{ serviceData.client_id || '-' }}</span>
            </div>
            <div class="info-row">
              <label>Transport</label>
              <span class="value">{{ serviceData.transport || 'HTTP' }}</span>
            </div>
          </div>
        </section>

        <!-- Health -->
        <section class="panel-section">
          <div class="panel-header">
            <h3 class="panel-title">
              Health
            </h3>
            <div class="panel-controls">
              <span class="hint-text">{{ statusHint }}</span>
            </div>
          </div>
          <div class="panel-body health-grid">
            <div class="health-card">
              <p class="label">状态</p>
              <p class="value mono">{{ getStatusLabel(serviceData.status) }}</p>
              <p class="hint">{{ statusHint || '---' }}</p>
            </div>
            <div class="health-card">
              <p class="label">错误率</p>
              <p :class="['value', 'mono', metricTone(serviceData.window_error_rate)]">
                {{ formatErrorRate(serviceData.window_error_rate) }}
              </p>
              <p class="hint">样本 {{ formatSampleSize(serviceData.sample_size) }}</p>
            </div>
            <div class="health-card">
              <p class="label">延迟 P95/P99</p>
              <p class="value mono">
                {{ formatLatency(serviceData.latency_p95) }} / {{ formatLatency(serviceData.latency_p99) }}
              </p>
              <p class="hint">单位毫秒</p>
            </div>
            <div class="health-card">
              <p class="label">下次重试</p>
              <p class="value mono">
                {{ formatRemainingLabel(serviceData.retry_in, serviceData.next_retry_time) }}
              </p>
              <p class="hint">{{ formatAbsoluteLabel(serviceData.next_retry_time) }}</p>
            </div>
            <div class="health-card">
              <p class="label">硬超时</p>
              <p class="value mono">
                {{ formatRemainingLabel(serviceData.hard_timeout_in, serviceData.hard_deadline) }}
              </p>
              <p class="hint">{{ formatAbsoluteLabel(serviceData.hard_deadline) }}</p>
            </div>
            <div class="health-card">
              <p class="label">租约剩余</p>
              <p class="value mono">
                {{ formatRemainingLabel(serviceData.lease_remaining, serviceData.lease_deadline) }}
              </p>
              <p class="hint">{{ formatAbsoluteLabel(serviceData.lease_deadline) }}</p>
            </div>
          </div>
        </section>

        <!-- Connection Card -->
        <section class="panel-section">
          <div class="panel-header">
            <h3 class="panel-title">
              Connection
            </h3>
            <div class="panel-controls">
              <el-button
                link
                size="small"
                @click="toggleEdit('connection')"
              >
                {{ editMode.connection ? 'Cancel' : 'Edit' }}
              </el-button>
              <el-button 
                v-if="editMode.connection" 
                type="primary" 
                link 
                size="small"
                @click="saveChanges('connection')"
              >
                Save
              </el-button>
            </div>
          </div>
           
          <div class="panel-body form-card">
            <!-- Edit Mode -->
            <div
              v-if="editMode.connection"
              class="edit-form"
            >
              <template v-if="!serviceData.command">
                <div class="form-group">
                  <label>Service URL</label>
                  <input
                    v-model="editForm.url"
                    class="atom-input full"
                  >
                </div>
              </template>
              <template v-else>
                <div class="form-group">
                  <label>Command</label>
                  <input
                    v-model="editForm.command"
                    class="atom-input full"
                  >
                </div>
                <div class="form-group">
                  <label>Arguments</label>
                  <input
                    v-model="editForm.argsString"
                    class="atom-input full"
                    placeholder="Space separated"
                  >
                </div>
                <div class="form-group">
                  <label>Working Directory</label>
                  <input
                    v-model="editForm.working_dir"
                    class="atom-input full"
                    placeholder="Optional"
                  >
                </div>
              </template>
            </div>

            <!-- View Mode -->
            <div
              v-else
              class="view-content"
            >
              <template v-if="!serviceData.command">
                <div class="info-row vertical">
                  <label>URL</label>
                  <span class="value code-box">{{ serviceData.url }}</span>
                </div>
              </template>
              <template v-else>
                <div class="info-row vertical">
                  <label>Command</label>
                  <span class="value code-box">$ {{ serviceData.command }}</span>
                </div>
                <div
                  v-if="serviceData.args?.length"
                  class="info-row vertical"
                >
                  <label>Arguments</label>
                  <span class="value code-box">{{ serviceData.args.join(' ') }}</span>
                </div>
                <div
                  v-if="serviceData.working_dir"
                  class="info-row"
                >
                  <label>CWD</label>
                  <span class="value mono">{{ serviceData.working_dir }}</span>
                </div>
              </template>
            </div>
          </div>
        </section>
      </div>

      <!-- Right Column: Actions & Tools -->
      <div class="panel-column right-col">
        <!-- Actions -->
        <section class="panel-section">
          <div class="panel-header">
            <h3 class="panel-title">
              Actions
            </h3>
          </div>
          <div class="panel-body actions-card">
            <button 
              class="action-btn primary" 
              :disabled="actionLoading.restart"
              @click="restartService"
            >
              <el-icon><Refresh /></el-icon> Restart
            </button>
            <button 
              class="action-btn warning" 
              :disabled="actionLoading.disconnect"
              @click="disconnectService"
            >
              <el-icon><SwitchButton /></el-icon> Disconnect
            </button>
            <button 
              class="action-btn danger" 
              :disabled="actionLoading.delete"
              @click="deleteService"
            >
              <el-icon><Delete /></el-icon> Delete
            </button>
          </div>
        </section>

        <!-- Tools List -->
        <section class="panel-section full-height">
          <div class="panel-header">
            <h3 class="panel-title">
              Tools ({{ serviceTools.length }})
            </h3>
          </div>
          <div class="panel-body tools-list">
            <div
              v-if="serviceTools.length === 0"
              class="empty-state"
            >
              No tools available.
            </div>
            <div 
              v-for="tool in serviceTools" 
              v-else 
              :key="tool.name" 
              class="tool-item"
              @click="executeTool(tool)"
            >
              <div class="tool-info">
                <span class="tool-name">{{ tool.name }}</span>
                <span class="tool-desc truncate">{{ tool.description || 'No description' }}</span>
              </div>
              <el-icon class="arrow">
                <ArrowRight />
              </el-icon>
            </div>
          </div>
        </section>
      </div>
    </div>
    
    <div
      v-else
      class="error-state"
    >
      <el-empty description="Service not found or error loading details" />
      <el-button @click="$router.back()">
        Go Back
      </el-button>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted, reactive } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import { ElMessage, ElMessageBox } from 'element-plus'
import { 
  ArrowLeft, Loading, Refresh, SwitchButton, Delete, ArrowRight 
} from '@element-plus/icons-vue'
import StatusBadge from '@/components/common/StatusBadge.vue'
import { getStatusMeta } from '@/utils/serviceStatus'
import { formatErrorRate, formatLatency, formatSampleSize, formatRemaining, formatAbsoluteTime, errorRateLevel } from '@/utils/healthMetrics'

const route = useRoute()
const router = useRouter()
const systemStore = useSystemStore()

// State
const pageLoading = ref(true)
const serviceData = ref(null)
const serviceTools = ref([])
const editMode = reactive({ connection: false })
const editForm = reactive({ url: '', command: '', argsString: '', working_dir: '' })
const actionLoading = reactive({ restart: false, disconnect: false, delete: false })
const now = ref(Date.now())
let heartbeatTimer = null

const serviceName = computed(() => route.params.serviceName)
const getStatusLabel = (status) => getStatusMeta(status).text
const metricTone = (rate) => {
  const level = errorRateLevel(rate)
  if (level === 'danger') return 'is-danger'
  if (level === 'warn') return 'is-warn'
  return ''
}
const formatRemainingLabel = (value, deadline) => formatRemaining(value, deadline, now.value)
const formatAbsoluteLabel = (deadline) => formatAbsoluteTime(deadline)
const statusHint = computed(() => {
  const status = serviceData.value?.status
  if (status === 'ready') return '已就绪，可用但仍在稳定性观察'
  if (status === 'healthy') return '运行稳定'
  if (status === 'degraded') return '性能下降，请关注错误率/延迟'
  if (status === 'half_open') return '半开试探阶段，避免频繁操作'
  if (status === 'circuit_open') return '已熔断，等待退避恢复'
  if (status === 'disconnected') return '连接已断开，可能需要手动处理'
  return ''
})

const mergeHealthInfo = (target, health) => {
  if (!target || !health) return
  target.status = health.status ?? target.status
  target.window_error_rate = health.window_error_rate ?? target.window_error_rate
  target.latency_p95 = health.latency_p95 ?? target.latency_p95
  target.latency_p99 = health.latency_p99 ?? target.latency_p99
  target.sample_size = health.sample_size ?? target.sample_size
  target.retry_in = health.retry_in ?? target.retry_in
  target.hard_timeout_in = health.hard_timeout_in ?? target.hard_timeout_in
  target.lease_remaining = health.lease_remaining ?? target.lease_remaining
  target.next_retry_time = health.next_retry_time ?? target.next_retry_time
  target.hard_deadline = health.hard_deadline ?? target.hard_deadline
  target.lease_deadline = health.lease_deadline ?? target.lease_deadline
}

// Methods
const fetchServiceDetail = async () => {
  try {
    pageLoading.value = true
    const { api } = await import('@/api')
    
    // api.store.getServiceInfo returns the extracted data object directly
    const data = await api.store.getServiceInfo(serviceName.value)
    let health = null
    try {
      health = await api.store.checkServiceHealth(serviceName.value)
    } catch (healthErr) {
      console.warn('获取健康详情失败:', healthErr?.message || healthErr)
    }
    if (health) mergeHealthInfo(data, health)
    
    if (data) {
      serviceData.value = data
      initEditForm()
      await fetchServiceTools()
    } else {
      throw new Error('No data returned')
    }
  } catch (error) {
    console.error('Fetch error:', error)
    ElMessage.error(`Failed to load service: ${error.message}`)
  } finally {
    pageLoading.value = false
  }
}

const fetchServiceTools = async () => {
  try {
    await systemStore.fetchTools()
    serviceTools.value = systemStore.tools.filter(t => t.service === serviceName.value)
  } catch (e) { /* ignore tools error */ }
}

const initEditForm = () => {
  if (!serviceData.value) return
  editForm.url = serviceData.value.url || ''
  editForm.command = serviceData.value.command || ''
  editForm.argsString = Array.isArray(serviceData.value.args) ? serviceData.value.args.join(' ') : ''
  editForm.working_dir = serviceData.value.working_dir || ''
}

const toggleEdit = (section) => {
  editMode[section] = !editMode[section]
  if (!editMode[section]) initEditForm()
}

const saveChanges = async (section) => {
  try {
    const { api } = await import('@/api')
    let updateData = {}
    
    if (serviceData.value.command) {
       updateData = {
         command: editForm.command,
         args: editForm.argsString.split(/\s+/).filter(Boolean),
         working_dir: editForm.working_dir
       }
    } else {
       updateData = { url: editForm.url }
    }
    
    const res = await api.store.patchService(serviceName.value, updateData)
    if (res.success) {
       ElMessage.success('Saved')
       editMode[section] = false
       await fetchServiceDetail()
    } else {
       throw new Error(res.message || 'Save failed')
    }
  } catch (e) {
    ElMessage.error(e.message)
  }
}

const restartService = async () => {
  try {
    await ElMessageBox.confirm(`Restart ${serviceName.value}?`, 'Confirm')
    actionLoading.restart = true
    const { api } = await import('@/api')
    const res = await api.store.restartService(serviceName.value)
    if (res.success) {
       ElMessage.success('Restarted')
       fetchServiceDetail()
    } else throw new Error(res.message)
  } catch (e) { 
    if(e !== 'cancel') ElMessage.error(e.message) 
  } finally {
    actionLoading.restart = false
  }
}

const disconnectService = async () => {
  try {
    await ElMessageBox.confirm(`Disconnect ${serviceName.value}?`, 'Confirm', { type: 'warning' })
    actionLoading.disconnect = true
    const { api } = await import('@/api')
    // Note: ensure api.monitoring exists or use store api if available
    const res = await api.monitoring?.gracefulDisconnect(serviceName.value) 
                || { success: false, message: 'API not available' }
    
    if (res.success) {
       ElMessage.success('Disconnected')
       fetchServiceDetail()
    } else throw new Error(res.message)
  } catch (e) {
    if(e !== 'cancel') ElMessage.error(e.message)
  } finally {
    actionLoading.disconnect = false
  }
}

const deleteService = async () => {
  try {
    await ElMessageBox.confirm(`Delete ${serviceName.value}? This cannot be undone.`, 'Delete', { type: 'error' })
    actionLoading.delete = true
    const { api } = await import('@/api')
    const res = await api.store.deleteService(serviceName.value)
    if (res.success) {
       ElMessage.success('Deleted')
       router.push('/for_store/list_services')
    } else throw new Error(res.message)
  } catch (e) {
    if(e !== 'cancel') ElMessage.error(e.message)
  } finally {
    actionLoading.delete = false
  }
}

const executeTool = (tool) => {
  router.push({
    path: '/for_store/call_tool',
    query: { toolName: tool.name, serviceName: serviceName.value }
  })
}

onMounted(() => {
  heartbeatTimer = setInterval(() => {
    now.value = Date.now()
  }, 1000)
  fetchServiceDetail()
})

onUnmounted(() => {
  if (heartbeatTimer) {
    clearInterval(heartbeatTimer)
    heartbeatTimer = null
  }
})
</script>

<style lang="scss" scoped>
.service-detail-container {
  max-width: 1200px;
  margin: 0 auto;
  padding: 20px;
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

.back-link {
  color: var(--text-secondary);
  font-size: 13px;
  &:hover { color: var(--text-primary); }
}

// Loading & Error
.loading-state, .error-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 60px;
  color: var(--text-secondary);
  gap: 16px;
}

// Layout
.main-layout {
  display: grid;
  grid-template-columns: 1.5fr 1fr;
  gap: 24px;
  align-items: start;
  
  @media (max-width: 900px) {
    grid-template-columns: 1fr;
  }
}

.panel-column {
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.panel-section {
  display: flex;
  flex-direction: column;
  gap: 12px;
  &.full-height { height: 100%; }
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  
  .panel-title {
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    color: var(--text-secondary);
    letter-spacing: 0.05em;
  }
}

.panel-body {
  background: var(--bg-surface);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: 20px;
}

// Cards Content
.status-badge {
  font-size: 10px;
  font-weight: 700;
  padding: 2px 8px;
  border-radius: 4px;
  background: var(--bg-hover);
  color: var(--text-secondary);
  
  &.healthy, &.active { background: #dcfce7; color: #166534; }
  &.error { background: #fee2e2; color: #991b1b; }
}

.status-badge.unknown {
  border: 1px solid var(--border-color);
}

.hint-text {
  font-size: 12px;
  color: var(--text-secondary);
}

.health-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
  gap: 12px;
}

.health-card {
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  padding: 12px;
  background: var(--bg-surface);

  .label {
    font-size: 12px;
    color: var(--text-secondary);
    margin-bottom: 4px;
  }

  .value {
    font-size: 16px;
    font-weight: 600;
    color: var(--text-primary);

    &.mono { font-family: var(--font-mono); }
    &.is-warn { color: var(--color-warning); }
    &.is-danger { color: var(--color-danger); }
  }

  .hint {
    margin-top: 4px;
    color: var(--text-secondary);
    font-size: 12px;
  }
}

.info-row {
  display: flex;
  justify-content: space-between;
  padding: 8px 0;
  border-bottom: 1px solid var(--bg-hover);
  font-size: 13px;
  
  &:last-child { border-bottom: none; }
  
  label { color: var(--text-secondary); font-weight: 500; }
  .value { color: var(--text-primary); font-weight: 500; text-align: right; word-break: break-all; }
  .mono { font-family: var(--font-mono); font-size: 12px; }
  
  .tag { 
    background: var(--bg-hover); 
    padding: 2px 6px; 
    border-radius: 4px; 
    font-size: 11px; 
    color: var(--text-primary);
  }
  
  &.vertical {
    flex-direction: column;
    gap: 6px;
    .value { text-align: left; }
  }
}

.code-box {
  background: var(--bg-body);
  padding: 8px 12px;
  border-radius: 6px;
  border: 1px solid var(--border-color);
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.4;
  color: var(--text-primary);
}

// Forms
.form-group {
  margin-bottom: 16px;
  label {
    display: block;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    margin-bottom: 6px;
  }
}

.atom-input {
  border: 1px solid var(--border-color);
  background: var(--bg-body);
  padding: 8px 12px;
  border-radius: 6px;
  font-size: 13px;
  color: var(--text-primary);
  box-sizing: border-box;
  
  &:focus { outline: none; border-color: var(--text-secondary); }
  &.full { width: 100%; }
}

// Actions
.actions-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.action-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 10px;
  border-radius: 6px;
  border: 1px solid var(--border-color);
  background: var(--bg-surface);
  color: var(--text-primary);
  font-weight: 500;
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s;
  
  &:hover { background: var(--bg-hover); }
  &:disabled { opacity: 0.6; cursor: not-allowed; }
  
  &.primary:hover { border-color: var(--color-success); color: var(--color-success); }
  &.warning:hover { border-color: var(--color-warning); color: var(--color-warning); }
  &.danger:hover { border-color: var(--color-danger); color: var(--color-danger); }
}

// Tools List
.tools-list {
  padding: 0;
  max-height: 400px;
  overflow-y: auto;
}

.tool-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-color);
  cursor: pointer;
  transition: background 0.2s;
  
  &:last-child { border-bottom: none; }
  &:hover { background: var(--bg-hover); .arrow { color: var(--text-primary); } }
  
  .tool-info {
    display: flex;
    flex-direction: column;
    min-width: 0;
    
    .tool-name { font-size: 13px; font-weight: 600; color: var(--text-primary); }
    .tool-desc { font-size: 11px; color: var(--text-secondary); }
  }
  
  .arrow { color: var(--text-placeholder); font-size: 14px; }
}

.truncate {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.empty-state {
  padding: 20px;
  text-align: center;
  font-size: 13px;
  color: var(--text-placeholder);
  font-style: italic;
}
</style>
