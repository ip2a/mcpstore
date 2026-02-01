<template>
  <div class="page-shell tool-records-container">
    <!-- Header -->
    <header class="page-header">
      <div class="header-content">
        <h1 class="page-title">
          Activity Log
        </h1>
        <p class="page-subtitle">
          History of all tool executions and outcomes
        </p>
      </div>
      <div class="header-actions">
        <el-button 
          :icon="Refresh" 
          :loading="loading" 
          circle 
          plain 
          class="refresh-btn"
          @click="reload" 
        />
      </div>
    </header>

    <!-- Main Content -->
    <section class="panel-section">
      <!-- Filters -->
      <div class="panel-header">
        <h3 class="panel-title">
          Records
        </h3>
        <div class="panel-controls">
          <div class="search-wrapper">
            <el-icon class="search-icon">
              <Search />
            </el-icon>
            <input
              v-model="filters.toolName"
              class="atom-input small"
              placeholder="Filter by tool..."
              @change="reload"
            >
          </div>
          <div class="search-wrapper">
            <el-icon class="search-icon">
              <Connection />
            </el-icon>
            <input
              v-model="filters.serviceName"
              class="atom-input small"
              placeholder="Filter by service..."
              @change="reload"
            >
          </div>
        </div>
      </div>

      <!-- Table -->
      <div class="panel-body table-container">
        <el-table
          v-loading="loading"
          :data="records"
          class="atom-table"
          size="small"
        >
          <el-table-column
            label="TIME"
            width="160"
          >
            <template #default="{ row }">
              <div class="time-cell">
                <span class="main-time">{{ formatTime(row.timestamp || row.created_at) }}</span>
                <span class="rel-time">{{ formatRelative(row.timestamp || row.created_at) }}</span>
              </div>
            </template>
          </el-table-column>

          <el-table-column
            label="TOOL / SERVICE"
            min-width="240"
          >
            <template #default="{ row }">
              <div class="identity-cell">
                <span class="tool-name">{{ row.tool_name }}</span>
                <span class="service-name">{{ row.service_name }}</span>
              </div>
            </template>
          </el-table-column>

          <el-table-column
            label="DURATION"
            width="100"
            align="right"
          >
            <template #default="{ row }">
              <span class="mono-text">{{ formatDuration(row) }}</span>
            </template>
          </el-table-column>

          <el-table-column
            label="STATUS"
            width="100"
            align="center"
          >
            <template #default="{ row }">
              <span :class="['status-badge', row.error ? 'error' : 'success']">
                {{ row.error ? 'FAIL' : 'OK' }}
              </span>
            </template>
          </el-table-column>

          <el-table-column
            label="PAYLOAD"
            min-width="200"
          >
            <template #default="{ row }">
              <span
                class="truncate-code"
                :title="formatJSON(row.params || row.args)"
              >
                {{ formatJSON(row.params || row.args) }}
              </span>
            </template>
          </el-table-column>

          <el-table-column
            label=""
            width="80"
            align="right"
          >
            <template #default="{ row }">
              <button
                class="text-btn"
                @click="viewDetails(row)"
              >
                View
              </button>
            </template>
          </el-table-column>
          
          <template #empty>
            <div class="empty-state">
              No records found.
            </div>
          </template>
        </el-table>
        
        <!-- Pagination -->
        <div class="pagination-bar">
          <el-pagination
            v-model:current-page="page"
            v-model:page-size="pageSize"
            :total="total"
            layout="prev, pager, next"
            small
            background
            @current-change="reload"
          />
        </div>
      </div>
    </section>

    <!-- Detail Dialog -->
    <el-dialog
      v-model="detailsVisible"
      title="Execution Details"
      width="600px"
      class="atom-dialog"
    >
      <div
        v-if="selectedRecord"
        class="detail-content"
      >
        <div class="detail-header">
          <div class="dh-item">
            <label>Tool</label>
            <span>{{ selectedRecord.tool_name }}</span>
          </div>
          <div class="dh-item">
            <label>Service</label>
            <span>{{ selectedRecord.service_name }}</span>
          </div>
          <div class="dh-item right">
            <span :class="['status-badge', selectedRecord.error ? 'error' : 'success']">
              {{ selectedRecord.error ? 'FAILURE' : 'SUCCESS' }}
            </span>
          </div>
        </div>
          
        <div class="detail-section">
          <label>Parameters</label>
          <pre class="code-block">{{ formatJSON(selectedRecord.params || selectedRecord.args, true) }}</pre>
        </div>
          
        <div class="detail-section">
          <label>Result / Error</label>
          <pre class="code-block">{{ formatJSON(selectedRecord.result || selectedRecord.response || selectedRecord.error, true) }}</pre>
        </div>
      </div>
      <template #footer>
        <el-button @click="detailsVisible = false">
          Close
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { api } from '@/api'
import dayjs from 'dayjs'
import relativeTime from 'dayjs/plugin/relativeTime'
import { Refresh, Search, Connection } from '@element-plus/icons-vue'

dayjs.extend(relativeTime)

// State
const records = ref([])
const loading = ref(false)
const page = ref(1)
const pageSize = ref(20)
const total = ref(0)
const filters = ref({ toolName: '', serviceName: '' })

const detailsVisible = ref(false)
const selectedRecord = ref(null)

// Methods
const reload = async () => {
  loading.value = true
  try {
    const params = {
      tool_name: filters.value.toolName || undefined,
      service_name: filters.value.serviceName || undefined,
      page: page.value,
      page_size: pageSize.value
    }
    const data = await api.store.getToolRecordsPaged(params)
    const list = data?.executions || data?.list || []
    const totalCount = data?.pagination?.total || data?.total || list.length
    
    records.value = list
    total.value = totalCount
  } finally {
    loading.value = false
  }
}

const viewDetails = (row) => {
  selectedRecord.value = row
  detailsVisible.value = true
}

// Formatters
const normalizeTs = (ts) => {
  if (!ts) return null
  const n = Number(ts)
  return (!isNaN(n) && n < 1e12) ? n * 1000 : (isNaN(n) ? ts : n)
}

const formatTime = (ts) => {
  const t = normalizeTs(ts)
  return t ? dayjs(t).format('HH:mm:ss') : '-'
}

const formatRelative = (ts) => {
  const t = normalizeTs(ts)
  return t ? dayjs(t).fromNow() : ''
}

const formatDuration = (row) => {
  const ms = row.elapsed_ms ?? row.duration_ms ?? row.response_time
  return ms ? `${ms}ms` : '-'
}

const formatJSON = (val, pretty = false) => {
  if (!val) return ''
  try {
    const obj = typeof val === 'string' ? JSON.parse(val) : val
    return JSON.stringify(obj, null, pretty ? 2 : 0)
  } catch {
    return String(val)
  }
}

onMounted(() => reload())
</script>

<style lang="scss" scoped>
.tool-records-container {
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

.refresh-btn {
  border-color: var(--border-color);
  color: var(--text-secondary);
  &:hover { color: var(--text-primary); border-color: var(--text-secondary); background: transparent; }
}

// Panel
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
    color: var(--text-secondary);
    letter-spacing: 0.05em;
  }
}

.panel-controls {
  display: flex;
  gap: 12px;
}

// Search Input
.search-wrapper {
  position: relative;
  .search-icon {
    position: absolute;
    left: 8px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-placeholder);
    font-size: 12px;
  }
  .atom-input {
    border: 1px solid var(--border-color);
    background: var(--bg-surface);
    padding: 4px 8px 4px 24px;
    border-radius: 4px;
    font-size: 12px;
    color: var(--text-primary);
    width: 160px;
    
    &:focus { outline: none; border-color: var(--text-secondary); }
  }
}

// Table
.table-container {
  background: var(--bg-surface);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  overflow: hidden;
  min-height: 400px;
  display: flex;
  flex-direction: column;
}

:deep(.atom-table) {
  flex: 1;
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
    padding: 8px 12px;
    text-transform: uppercase;
  }

  td.el-table__cell {
    border-bottom: 1px solid var(--border-color) !important;
    padding: 8px 12px;
  }
  
  .el-table__inner-wrapper::before { display: none; }
}

// Cells
.time-cell {
  display: flex;
  flex-direction: column;
  .main-time { font-size: 13px; color: var(--text-primary); font-family: var(--font-mono); }
  .rel-time { font-size: 11px; color: var(--text-placeholder); }
}

.identity-cell {
  display: flex;
  flex-direction: column;
  .tool-name { font-size: 13px; font-weight: 500; color: var(--text-primary); }
  .service-name { font-size: 11px; color: var(--text-secondary); }
}

.mono-text {
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--text-secondary);
}

.status-badge {
  font-size: 10px;
  font-weight: 700;
  padding: 2px 6px;
  border-radius: 4px;
  
  &.success { background: #dcfce7; color: #166534; }
  &.error { background: #fee2e2; color: #991b1b; }
}

.truncate-code {
  display: block;
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100%;
}

.text-btn {
  background: none;
  border: none;
  cursor: pointer;
  font-size: 12px;
  color: var(--text-secondary);
  padding: 0;
  &:hover { color: var(--text-primary); text-decoration: underline; }
}

.empty-state {
  padding: 40px;
  text-align: center;
  color: var(--text-placeholder);
  font-size: 13px;
  font-style: italic;
}

.pagination-bar {
  padding: 8px;
  border-top: 1px solid var(--border-color);
  display: flex;
  justify-content: flex-end;
}

// Dialog
.detail-content {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.detail-header {
  display: flex;
  gap: 24px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--border-color);
  
  .dh-item {
    display: flex;
    flex-direction: column;
    label { font-size: 10px; font-weight: 700; color: var(--text-secondary); text-transform: uppercase; margin-bottom: 2px; }
    span { font-size: 13px; font-weight: 500; color: var(--text-primary); }
    
    &.right { margin-left: auto; align-items: flex-end; justify-content: center; }
  }
}

.detail-section {
  label {
    display: block;
    font-size: 11px;
    font-weight: 700;
    color: var(--text-secondary);
    text-transform: uppercase;
    margin-bottom: 6px;
  }
}

.code-block {
  background: var(--bg-body);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  padding: 12px;
  margin: 0;
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.5;
  overflow: auto;
  max-height: 200px;
  color: var(--text-primary);
}
</style>
