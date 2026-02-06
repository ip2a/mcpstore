<template>
  <div class="page-shell tool-list-container">
    <!-- Header -->
    <header class="page-header">
      <div class="header-content">
        <h1 class="page-title">
          Tools Library
        </h1>
        <p class="page-subtitle">
          {{ serviceFilter ? `Tools provided by "${serviceFilter}"` : 'Browse and execute available MCP tools' }}
        </p>
      </div>
      <div class="header-actions">
        <el-button
          v-if="serviceFilter"
          link
          class="action-link"
          @click="clearServiceFilter"
        >
          Clear Filter
        </el-button>
        <el-button
          :icon="Refresh"
          :loading="loading"
          circle
          plain
          class="refresh-btn"
          @click="refreshTools"
        />
      </div>
    </header>

    <!-- KPI Grid -->
    <div class="kpi-grid">
      <StatCard
        title="Total Tools"
        :value="systemStore.stats.totalTools"
        unit="fns"
        :icon="Tools"
        class="kpi-card"
      />
      <StatCard
        title="Active Services"
        :value="systemStore.stats.totalServices"
        unit="svcs"
        :icon="Connection"
        class="kpi-card"
      />
      <StatCard
        title="Categories"
        :value="Object.keys(toolsByService).length"
        unit="types"
        :icon="Menu"
        class="kpi-card"
      />
    </div>

    <!-- Main Content -->
    <section class="panel-section">
      <!-- Controls -->
      <div class="panel-header">
        <h3 class="panel-title">
          Available Functions
        </h3>
        <div class="panel-controls">
          <div class="search-wrapper">
            <el-icon class="search-icon">
              <Search />
            </el-icon>
            <input 
              v-model="searchQuery" 
              class="atom-input search-input" 
              placeholder="Search tools..."
            >
          </div>
          <select
            v-model="serviceFilter"
            class="atom-input filter-select"
          >
            <option value="">
              All Services
            </option>
            <option
              v-for="name in serviceNames"
              :key="name"
              :value="name"
            >
              {{ name }}
            </option>
          </select>
        </div>
      </div>

      <!-- Table -->
      <div class="panel-body table-container">
        <el-table 
          v-loading="loading"
          :data="filteredTools" 
          class="atom-table" 
          :show-header="true" 
          size="small"
        >
          <el-table-column
            prop="name"
            label="TOOL NAME"
            min-width="200"
          >
            <template #default="{ row }">
              <span class="primary-text font-medium">{{ row.name }}</span>
            </template>
          </el-table-column>
          
          <el-table-column
            prop="service"
            label="SERVICE"
            width="160"
          >
            <template #default="{ row }">
              <span class="secondary-text">{{ row.service }}</span>
            </template>
          </el-table-column>

          <el-table-column
            prop="description"
            label="DESCRIPTION"
            min-width="300"
          >
            <template #default="{ row }">
              <span
                class="secondary-text truncate-multiline"
                :title="row.description"
              >
                {{ row.description || '-' }}
              </span>
            </template>
          </el-table-column>

          <el-table-column
            label="INPUTS"
            min-width="200"
          >
            <template #default="{ row }">
              <div class="inputs-preview">
                <span
                  v-if="!getInputsList(row).length"
                  class="no-inputs"
                >No inputs</span>
                <span 
                  v-for="(input, idx) in getInputsList(row).slice(0, 3)" 
                  :key="idx" 
                  class="input-tag"
                >
                  {{ input.key }}
                </span>
                <span
                  v-if="getInputsList(row).length > 3"
                  class="input-tag more"
                >
                  +{{ getInputsList(row).length - 3 }}
                </span>
              </div>
            </template>
          </el-table-column>

          <el-table-column
            label=""
            width="140"
            align="right"
          >
            <template #default="{ row }">
              <div class="row-actions">
                <button
                  class="text-btn"
                  @click="viewToolDetails(row)"
                >
                  Details
                </button>
                <button
                  class="text-btn primary"
                  @click="executeTool(row)"
                >
                  Run
                </button>
              </div>
            </template>
          </el-table-column>
          
          <template #empty>
            <div class="empty-state">
              <span class="empty-text">No tools found</span>
            </div>
          </template>
        </el-table>
      </div>
    </section>

    <!-- Details Dialog -->
    <el-dialog
      v-model="detailDialogVisible"
      :title="selectedTool?.name"
      width="600px"
      class="atom-dialog"
      align-center
    >
      <div
        v-if="selectedTool"
        class="dialog-content"
      >
        <div class="detail-group">
          <label>Service</label>
          <p>{{ selectedTool.service }}</p>
        </div>
        <div class="detail-group">
          <label>Description</label>
          <p>{{ selectedTool.description || 'No description provided.' }}</p>
        </div>
        <div class="detail-group">
          <label>Schema</label>
          <pre class="code-block">{{ JSON.stringify(selectedTool.input_schema, null, 2) }}</pre>
        </div>
      </div>
      <template #footer>
        <div class="dialog-footer">
          <el-button @click="detailDialogVisible = false">
            Close
          </el-button>
          <el-button
            type="primary"
            color="#000"
            @click="executeTool(selectedTool)"
          >
            Execute Tool
          </el-button>
        </div>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import { schemaToList } from '@/utils/schema'
import { Refresh, Tools, Connection, Menu, Search } from '@element-plus/icons-vue'
import StatCard from '@/components/common/StatCard.vue'
import { ElMessage } from 'element-plus'

const router = useRouter()
const route = useRoute()
const systemStore = useSystemStore()

const loading = ref(false)
const searchQuery = ref('')
const serviceFilter = ref('')
const detailDialogVisible = ref(false)
const selectedTool = ref(null)

const toolsByService = computed(() => systemStore.toolsByService)

const serviceNames = computed(() => {
  const names = new Set(systemStore.tools.map(tool => tool.service))
  return Array.from(names).sort()
})

const filteredTools = computed(() => {
  let tools = systemStore.tools

  if (searchQuery.value) {
    const query = searchQuery.value.toLowerCase()
    tools = tools.filter(tool =>
      tool.name.toLowerCase().includes(query) ||
      (tool.description && tool.description.toLowerCase().includes(query))
    )
  }

  if (serviceFilter.value) {
    tools = tools.filter(tool => tool.service === serviceFilter.value)
  }

  return tools
})

const refreshTools = async () => {
  loading.value = true
  try {
    await systemStore.fetchTools()
    ElMessage.success('Refreshed')
  } catch (error) {
    ElMessage.error('Failed to refresh')
  } finally {
    loading.value = false
  }
}

const clearServiceFilter = () => {
  serviceFilter.value = ''
}

const getInputsList = (tool) => {
  try {
    return schemaToList(tool?.input_schema || {})
  } catch {
    return []
  }
}

const executeTool = (tool) => {
  detailDialogVisible.value = false // close dialog if open
  router.push({
    path: '/for_store/call_tool',
    query: { 
      toolName: tool.name,
      serviceName: tool.service
    }
  })
}

const viewToolDetails = (tool) => {
  selectedTool.value = tool
  detailDialogVisible.value = true
}

onMounted(async () => {
  const serviceParam = route.query.service
  if (serviceParam) {
    serviceFilter.value = serviceParam
  }
  if (systemStore.tools.length === 0) {
      await refreshTools()
  }
})
</script>

<style lang="scss" scoped>
.tool-list-container {
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

.action-link {
  font-size: 13px;
  color: var(--text-secondary);
  &:hover { color: var(--text-primary); }
}

.refresh-btn {
  border-color: var(--border-color);
  color: var(--text-secondary);
  &:hover { 
    color: var(--text-primary);
    border-color: var(--text-secondary);
    background: transparent;
  }
}

// KPI Grid
.kpi-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 16px;
  margin-bottom: 24px;

  @media (max-width: 768px) { grid-template-columns: 1fr; }
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
  gap: 12px;
}

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
  
  .search-input {
    padding-left: 28px;
    width: 240px;
  }
}

// Atomic Inputs
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
}

.filter-select {
  width: 160px;
  cursor: pointer;
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

.primary-text {
  font-size: 13px;
  color: var(--text-primary);
  &.font-medium { font-weight: 500; }
}

.secondary-text {
  font-size: 13px;
  color: var(--text-secondary);
}

.truncate-multiline {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  text-overflow: ellipsis;
  line-height: 1.4;
  font-size: 12px;
  color: var(--text-secondary);
}

// Input Tags
.inputs-preview {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.input-tag {
  font-size: 11px;
  font-family: var(--font-mono);
  color: var(--text-secondary);
  background: var(--bg-body);
  border: 1px solid var(--border-color);
  padding: 2px 6px;
  border-radius: 4px;
  
  &.more {
    color: var(--text-placeholder);
    border-style: dashed;
  }
}

.no-inputs {
  font-size: 11px;
  color: var(--text-placeholder);
  font-style: italic;
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
  
  &.primary {
    color: var(--text-primary);
    &:hover { color: var(--color-accent); }
  }
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
  gap: 20px;
}

.detail-group {
  label {
    display: block;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    color: var(--text-secondary);
    margin-bottom: 8px;
    letter-spacing: 0.05em;
  }
  
  p {
    font-size: 13px;
    color: var(--text-primary);
    line-height: 1.5;
    margin: 0;
  }
}

.code-block {
  background: var(--bg-body);
  border: 1px solid var(--border-color);
  padding: 12px;
  border-radius: 6px;
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--text-primary);
  overflow: auto;
  max-height: 300px;
  margin: 0;
}
</style>
