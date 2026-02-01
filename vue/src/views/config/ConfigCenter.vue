<template>
  <div class="page-shell config-center-container">
    <!-- Header -->
    <header class="page-header">
      <div class="header-content">
        <h1 class="page-title">
          Configuration
        </h1>
        <p class="page-subtitle">
          Manage system and service configurations (mcp.json)
        </p>
      </div>
      <div class="header-actions">
        <el-button 
          :icon="Upload" 
          type="primary" 
          color="#000"
          class="import-btn"
          @click="showImportDialog"
        >
          Import
        </el-button>
      </div>
    </header>

    <!-- Main Layout -->
    <div class="main-layout">
      <!-- Left: File Browser & Stats -->
      <div class="panel-column left-col">
        <!-- File List -->
        <section class="panel-section">
          <div class="panel-header">
            <h3 class="panel-title">
              Files
            </h3>
            <div class="panel-controls">
              <div class="search-wrapper">
                <el-icon class="search-icon">
                  <Search />
                </el-icon>
                <input
                  v-model="searchQuery"
                  class="atom-input small"
                  placeholder="Search..."
                >
              </div>
            </div>
          </div>
          <div class="panel-body files-list">
            <div 
              v-for="file in filteredConfigFiles" 
              :key="file.name"
              class="file-item" 
              :class="{ active: selectedFile?.name === file.name }"
              @click="selectFile(file)"
            >
              <el-icon class="file-icon">
                <Document />
              </el-icon>
              <div class="file-info">
                <span class="name">{{ file.name }}</span>
                <span class="meta">{{ file.type.toUpperCase() }}</span>
              </div>
            </div>
          </div>
          <div class="panel-footer">
            <span>{{ configFiles.length }} files</span>
          </div>
        </section>
         
        <!-- Stats -->
        <section
          v-if="isMcpJson && hasServices"
          class="panel-section"
        >
          <div class="panel-header">
            <h3 class="panel-title">
              Stats
            </h3>
          </div>
          <div class="panel-body stats-card">
            <div class="stat-row">
              <span class="label">Total Services</span>
              <span class="value">{{ serviceCount }}</span>
            </div>
            <div class="divider" />
            <div class="service-tags">
              <span
                v-for="svc in servicesList.slice(0, 10)"
                :key="svc.name"
                class="tag"
              >
                {{ svc.name }}
              </span>
              <span
                v-if="servicesList.length > 10"
                class="tag more"
              >+{{ servicesList.length - 10 }}</span>
            </div>
          </div>
        </section>
      </div>

      <!-- Right: Editor -->
      <div class="panel-column right-col">
        <div
          v-if="selectedFile"
          class="editor-panel"
        >
          <div class="editor-toolbar">
            <div class="file-status">
              <span class="filename">{{ selectedFile.name }}</span>
              <span
                v-if="isModified"
                class="status-badge modified"
              >MODIFIED</span>
              <span
                v-if="hasErrors"
                class="status-badge error"
              >SYNTAX ERROR</span>
            </div>
               
            <div class="editor-actions">
              <template v-if="isMcpJson">
                <button
                  class="text-btn"
                  :disabled="!hasServices"
                  @click="showServicePreviewDialog"
                >
                  Preview
                </button>
                <button
                  class="text-btn danger"
                  @click="clearConfig"
                >
                  Clear
                </button>
                <button
                  class="text-btn"
                  @click="resetConfig"
                >
                  Reset
                </button>
              </template>
              <button
                class="text-btn"
                @click="exportConfig"
              >
                Export
              </button>
              <button
                class="text-btn primary"
                :disabled="hasErrors || !isModified"
                @click="saveConfig"
              >
                Save Changes
              </button>
            </div>
          </div>
            
          <div class="editor-area">
            <textarea 
              v-model="configText"
              class="code-editor" 
              spellcheck="false"
              @input="onConfigChange"
            />
          </div>
        </div>
         
        <div
          v-else
          class="empty-state"
        >
          <el-icon class="icon">
            <Document />
          </el-icon>
          <p>Select a configuration file to edit.</p>
        </div>
      </div>
    </div>

    <!-- Import Dialog -->
    <el-dialog
      v-model="importDialogVisible"
      title="Import Configuration"
      width="400px"
      class="atom-dialog"
    >
      <div class="dialog-content">
        <p class="dialog-desc">
          Importing will overwrite your current configuration. Only valid JSON files are accepted.
        </p>
        <div class="upload-area">
          <input
            ref="fileInput"
            type="file"
            accept=".json"
            @change="handleFileChange"
          >
        </div>
      </div>
      <template #footer>
        <div class="dialog-footer">
          <el-button @click="importDialogVisible = false">
            Cancel
          </el-button>
          <el-button
            type="primary"
            color="#000"
            :disabled="!selectedImportFile"
            @click="confirmImport"
          >
            Import
          </el-button>
        </div>
      </template>
    </el-dialog>

    <!-- Preview Dialog -->
    <el-dialog
      v-model="previewDialogVisible"
      title="Services Preview"
      width="600px"
      class="atom-dialog"
    >
      <div class="preview-list">
        <div
          v-for="(svc, idx) in servicesList"
          :key="idx"
          class="preview-item"
        >
          <div class="item-header">
            <span class="idx">{{ idx + 1 }}.</span>
            <span class="name">{{ svc.name }}</span>
          </div>
          <div class="item-details">
            <div
              v-if="svc.url"
              class="detail"
            >
              <span class="lbl">URL:</span> {{ svc.url }}
            </div>
            <div
              v-if="svc.command"
              class="detail"
            >
              <span class="lbl">CMD:</span> {{ svc.command }} {{ (svc.args||[]).join(' ') }}
            </div>
          </div>
        </div>
      </div>
      <template #footer>
        <el-button @click="previewDialogVisible = false">
          Close
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { api } from '@/api'
import { Document, Search, Upload } from '@element-plus/icons-vue'

// State
const searchQuery = ref('')
const selectedFile = ref(null)
const configText = ref('')
const originalConfigText = ref('')
const isModified = ref(false)
const hasErrors = ref(false)
const saving = ref(false)

// Dialogs
const importDialogVisible = ref(false)
const selectedImportFile = ref(null)
const previewDialogVisible = ref(false)
const fileInput = ref(null)

const configFiles = ref([{ name: 'mcp.json', type: 'json', path: '/config/mcp.json' }])

// Computed
const filteredConfigFiles = computed(() => {
  if (!searchQuery.value) return configFiles.value
  return configFiles.value.filter(f => f.name.toLowerCase().includes(searchQuery.value.toLowerCase()))
})

const isMcpJson = computed(() => selectedFile.value?.name === 'mcp.json')

const parsedConfig = computed(() => {
  try { return JSON.parse(configText.value) } catch { return null }
})

const servicesList = computed(() => {
  if (!parsedConfig.value?.mcpServers) return []
  return Object.entries(parsedConfig.value.mcpServers).map(([k, v]) => ({ name: k, ...v }))
})

const serviceCount = computed(() => servicesList.value.length)
const hasServices = computed(() => serviceCount.value > 0)

// Methods
const selectFile = async (file) => {
  selectedFile.value = file
  if (file.name === 'mcp.json') await loadMcpJson()
  else { configText.value = ''; originalConfigText.value = '' }
}

const loadMcpJson = async () => {
  try {
    const data = await api.store.getMcpJson()
    configText.value = typeof data === 'string' ? data : JSON.stringify(data, null, 2)
    originalConfigText.value = configText.value
    isModified.value = false
    validateJson()
  } catch (e) {
    ElMessage.error('Failed to load config')
  }
}

const onConfigChange = () => {
  isModified.value = configText.value !== originalConfigText.value
  validateJson()
}

const validateJson = () => {
  try {
    if (configText.value.trim()) { JSON.parse(configText.value); hasErrors.value = false }
  } catch { hasErrors.value = true }
}

const saveConfig = async () => {
  if (hasErrors.value) return ElMessage.error('Fix syntax errors first')
  try {
    await ElMessageBox.confirm('Save changes? This will reload services.', 'Confirm')
    saving.value = true
    const config = JSON.parse(configText.value)
    await api.store.resetMcpJson(config)
    ElMessage.success('Saved')
    originalConfigText.value = configText.value
    isModified.value = false
  } catch (e) {
     if(e !== 'cancel') ElMessage.error('Save failed')
  } finally {
     saving.value = false
  }
}

const clearConfig = async () => {
  try {
    await ElMessageBox.confirm('Clear configuration?', 'Warning', { type: 'warning' })
    configText.value = JSON.stringify({ mcpServers: {} }, null, 2)
    isModified.value = true
  } catch (e) { /* cancelled */ }
}

const resetConfig = async () => {
  try {
    await ElMessageBox.confirm('Discard changes and reload?', 'Warning', { type: 'warning' })
    await loadMcpJson()
    ElMessage.success('Reset')
  } catch (e) { /* cancelled */ }
}

const exportConfig = () => {
  const blob = new Blob([configText.value], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = selectedFile.value?.name || 'config.json'
  a.click()
}

const showImportDialog = () => {
  importDialogVisible.value = true
  selectedImportFile.value = null
  if(fileInput.value) fileInput.value.value = ''
}

const handleFileChange = (e) => {
  selectedImportFile.value = e.target.files[0]
}

const confirmImport = () => {
  if (!selectedImportFile.value) return
  const reader = new FileReader()
  reader.onload = (e) => {
    try {
      JSON.parse(e.target.result)
      configText.value = e.target.result
      isModified.value = true
      importDialogVisible.value = false
      ElMessage.success('Imported')
      if(!selectedFile.value) selectFile(configFiles.value[0])
    } catch {
      ElMessage.error('Invalid JSON')
    }
  }
  reader.readAsText(selectedImportFile.value)
}

const showServicePreviewDialog = () => previewDialogVisible.value = true

onMounted(() => {
  if(configFiles.value.length) selectFile(configFiles.value[0])
})
</script>

<style lang="scss" scoped>
.config-center-container {
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

.import-btn {
  font-weight: 500;
  border-radius: 6px;
}

// Main Layout
.main-layout {
  display: grid;
  grid-template-columns: 300px 1fr;
  gap: 24px;
  height: calc(100vh - 200px);
  min-height: 500px;
  
  @media (max-width: 768px) {
    grid-template-columns: 1fr;
    height: auto;
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
  }
}

.panel-body {
  background: var(--bg-surface);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.panel-footer {
  font-size: 11px;
  color: var(--text-placeholder);
  text-align: center;
  margin-top: 8px;
}

// Files List
.files-list {
  padding: 8px;
}

.file-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
  
  &:hover { background: var(--bg-hover); }
  &.active { background: var(--bg-hover); border: 1px solid var(--border-color); }
  
  .file-icon { font-size: 18px; color: var(--text-secondary); }
  
  .file-info {
    display: flex;
    flex-direction: column;
    
    .name { font-size: 13px; font-weight: 500; color: var(--text-primary); }
    .meta { font-size: 10px; color: var(--text-placeholder); }
  }
}

// Search
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
    background: var(--bg-body);
    padding: 4px 8px 4px 24px;
    border-radius: 4px;
    font-size: 12px;
    color: var(--text-primary);
    width: 140px;
    
    &:focus { outline: none; border-color: var(--text-secondary); }
  }
}

// Stats
.stats-card {
  padding: 16px;
  
  .stat-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    
    .label { font-size: 12px; color: var(--text-secondary); }
    .value { font-size: 14px; font-weight: 600; color: var(--text-primary); }
  }
  
  .divider { height: 1px; background: var(--border-color); margin: 12px 0; }
  
  .service-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    
    .tag {
      font-size: 11px;
      background: var(--bg-hover);
      padding: 2px 6px;
      border-radius: 4px;
      color: var(--text-secondary);
      
      &.more { background: transparent; border: 1px dashed var(--border-color); }
    }
  }
}

// Editor
.editor-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--bg-surface);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.editor-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-color);
  
  .file-status {
    display: flex;
    align-items: center;
    gap: 12px;
    
    .filename { font-weight: 600; font-size: 14px; color: var(--text-primary); }
    
    .status-badge {
      font-size: 10px;
      font-weight: 700;
      padding: 2px 6px;
      border-radius: 4px;
      
      &.modified { background: #fff7ed; color: #c2410c; }
      &.error { background: #fee2e2; color: #991b1b; }
    }
  }
  
  .editor-actions {
    display: flex;
    gap: 12px;
  }
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
  &.danger:hover { color: var(--color-danger); }
  &.primary { color: var(--text-primary); font-weight: 600; }
  &:disabled { opacity: 0.5; cursor: not-allowed; text-decoration: none; }
}

.editor-area {
  flex: 1;
  position: relative;
}

.code-editor {
  width: 100%;
  height: 100%;
  border: none;
  resize: none;
  padding: 16px;
  font-family: var(--font-mono);
  font-size: 13px;
  line-height: 1.5;
  color: var(--text-primary);
  background: var(--bg-body);
  box-sizing: border-box;
  
  &:focus { outline: none; }
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--text-placeholder);
  background: var(--bg-surface);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  
  .icon { font-size: 48px; margin-bottom: 16px; }
}

// Dialogs
.dialog-content {
  .dialog-desc { font-size: 13px; color: var(--text-secondary); margin-bottom: 16px; }
  .upload-area { margin-bottom: 8px; }
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.preview-list {
  max-height: 400px;
  overflow-y: auto;
  
  .preview-item {
    padding: 12px 0;
    border-bottom: 1px solid var(--border-color);
    &:last-child { border-bottom: none; }
    
    .item-header {
      margin-bottom: 6px;
      font-size: 13px;
      .idx { color: var(--color-accent); margin-right: 8px; font-weight: 600; }
      .name { font-weight: 600; }
    }
    
    .item-details {
      padding-left: 20px;
      font-size: 12px;
      color: var(--text-secondary);
      font-family: var(--font-mono);
      
      .detail { margin-bottom: 2px; }
      .lbl { color: var(--text-placeholder); }
    }
  }
}
</style>
