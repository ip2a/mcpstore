<template>
  <div class="page-shell tool-execute-container">
    <!-- Header -->
    <header class="page-header">
      <div class="header-content">
        <h1 class="page-title">
          Tool Executor
        </h1>
        <p class="page-subtitle">
          Interactive tool invocation and testing environment
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

    <!-- Main Content -->
    <div class="main-layout">
      <!-- Left Panel: Configuration -->
      <div class="panel-column left-col">
        <!-- Selection -->
        <section class="panel-section">
          <div class="panel-header">
            <h3 class="panel-title">
              Target
            </h3>
          </div>
          <div class="panel-body form-card">
            <div class="form-row">
              <div class="form-group">
                <label>Service</label>
                <select 
                  v-model="selectedService" 
                  class="atom-input full"
                  @change="handleServiceChange"
                >
                  <option
                    value=""
                    disabled
                    selected
                  >
                    Select Service
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
              <div class="form-group">
                <label>Tool</label>
                <select 
                  v-model="selectedTool" 
                  class="atom-input full"
                  :disabled="!selectedService"
                  @change="handleToolChange"
                >
                  <option
                    value=""
                    disabled
                    selected
                  >
                    Select Tool
                  </option>
                  <option
                    v-for="tool in availableTools"
                    :key="tool.name"
                    :value="tool.name"
                  >
                    {{ tool.name }}
                  </option>
                </select>
              </div>
            </div>
            
            <div
              v-if="currentTool"
              class="tool-summary"
            >
              <p class="description">
                {{ currentTool.description || 'No description available.' }}
              </p>
            </div>
          </div>
        </section>

        <!-- Parameters -->
        <section
          v-if="currentTool"
          class="panel-section"
        >
          <div class="panel-header">
            <h3 class="panel-title">
              Parameters
            </h3>
            <div class="panel-controls">
              <span 
                class="mode-switch" 
                :class="{ active: !useJsonMode }"
                @click="useJsonMode = false"
              >Form</span>
              <span class="divider">/</span>
              <span 
                class="mode-switch" 
                :class="{ active: useJsonMode }"
                @click="useJsonMode = true"
              >JSON</span>
               
              <div class="spacer" />
               
              <el-button
                link
                size="small"
                @click="resetParams"
              >
                Reset
              </el-button>
              <el-button
                link
                size="small"
                @click="loadExample"
              >
                Example
              </el-button>
            </div>
          </div>

          <div class="panel-body params-container">
            <!-- Form Mode -->
            <div
              v-if="!useJsonMode && hasParameters"
              class="form-mode"
            >
              <el-form
                ref="paramsFormRef"
                :model="toolParams"
                :rules="paramRules"
                label-position="top"
              >
                <div class="params-grid">
                  <div
                    v-for="(param, name) in toolParameters"
                    :key="name"
                    class="param-field"
                  >
                    <label class="param-label">
                      {{ name }}
                      <span
                        v-if="isRequired(name)"
                        class="required"
                      >*</span>
                      <span class="param-type">{{ param.type }}</span>
                    </label>
                       
                    <!-- Inputs -->
                    <input 
                      v-if="param.type === 'string'"
                      v-model="toolParams[name]"
                      class="atom-input full"
                      :placeholder="param.description"
                    >
                       
                    <input 
                      v-else-if="param.type === 'integer' || param.type === 'number'"
                      v-model.number="toolParams[name]"
                      type="number"
                      class="atom-input full"
                    >
                       
                    <div
                      v-else-if="param.type === 'boolean'"
                      class="checkbox-wrapper"
                    >
                      <el-switch
                        v-model="toolParams[name]"
                        size="small"
                      />
                      <span class="checkbox-label">{{ toolParams[name] ? 'True' : 'False' }}</span>
                    </div>
                       
                    <textarea
                      v-else
                      v-model="toolParams[name]"
                      class="atom-input full"
                      rows="3"
                      placeholder="Complex input (JSON/Array)"
                    />
                       
                    <p
                      v-if="param.description"
                      class="help-text"
                    >
                      {{ param.description }}
                    </p>
                  </div>
                </div>
              </el-form>
            </div>
             
            <!-- JSON Mode -->
            <div
              v-else-if="useJsonMode"
              class="json-mode"
            >
              <textarea 
                v-model="jsonInput" 
                class="code-editor" 
                rows="12"
                placeholder="{ ... }"
              />
              <div class="json-actions">
                <button
                  class="text-btn"
                  @click="formatJson"
                >
                  Format
                </button>
                <button
                  class="text-btn"
                  @click="pasteFromClipboard"
                >
                  Paste
                </button>
              </div>
            </div>
             
            <!-- No Params -->
            <div
              v-else
              class="empty-params"
            >
              <span>No parameters required for this tool.</span>
            </div>
          </div>
        </section>
      </div>

      <!-- Right Panel: Execution & Output -->
      <div class="panel-column right-col">
        <!-- Action -->
        <div class="action-card">
          <div class="action-info">
            <span class="target-label">{{ selectedTool ? `${selectedService} / ${selectedTool}` : 'No tool selected' }}</span>
            <span
              class="status-indicator"
              :class="{ ready: canExecute }"
            />
          </div>
          <el-button 
            type="primary" 
            color="#000" 
            size="large" 
            :loading="executing"
            :disabled="!canExecute"
            class="execute-btn"
            @click="executeTool"
          >
            {{ executing ? 'Running...' : 'Run Tool' }}
          </el-button>
        </div>

        <!-- Output Tabs -->
        <div class="output-section">
          <div class="tabs-header">
            <div 
              class="tab-item" 
              :class="{ active: activeTab === 'result' }"
              @click="activeTab = 'result'"
            >
              Result
            </div>
            <div 
              class="tab-item" 
              :class="{ active: activeTab === 'request' }"
              @click="activeTab = 'request'"
            >
              Payload
            </div>
            <div 
              class="tab-item" 
              :class="{ active: activeTab === 'history' }"
              @click="activeTab = 'history'"
            >
              History
            </div>
          </div>
           
          <div class="tab-content">
            <!-- Result Tab -->
            <div
              v-if="activeTab === 'result'"
              class="result-view"
            >
              <div
                v-if="executionResult"
                class="result-meta"
              >
                <span :class="['status-tag', executionResult.success ? 'success' : 'error']">
                  {{ executionResult.success ? 'SUCCESS' : 'ERROR' }}
                </span>
                <span class="meta-item">{{ executionResult.execution_info?.duration_ms }}ms</span>
                <div class="meta-actions">
                  <button
                    class="text-btn"
                    @click="copyResult"
                  >
                    Copy
                  </button>
                </div>
              </div>
                 
              <div
                v-if="executionResult"
                class="result-body"
              >
                <pre v-if="formattedResult">{{ formattedResult }}</pre>
                <div
                  v-else
                  class="raw-output"
                >
                  {{ executionResult.data }}
                </div>
              </div>
                 
              <div
                v-else
                class="empty-state"
              >
                Ready to execute.
              </div>
            </div>
              
            <!-- Request Tab -->
            <div
              v-if="activeTab === 'request'"
              class="request-view"
            >
              <pre class="code-block">{{ requestPayload ? JSON.stringify(requestPayload, null, 2) : '// No request payload yet' }}</pre>
            </div>
              
            <!-- History Tab -->
            <div
              v-if="activeTab === 'history'"
              class="history-view"
            >
              <div
                v-if="historyRecords.length === 0"
                class="empty-state"
              >
                No history.
              </div>
              <div
                v-else
                class="history-list"
              >
                <div
                  v-for="(rec, idx) in historyRecords"
                  :key="idx"
                  class="history-item"
                >
                  <div class="history-main">
                    <span class="hist-name">{{ rec.tool_name }}</span>
                    <span class="hist-time">{{ rec.response_time }}ms</span>
                  </div>
                  <div class="history-actions">
                    <span :class="['status-dot', rec.error ? 'error' : 'success']" />
                    <button
                      class="text-btn"
                      @click="rerun(rec)"
                    >
                      Rerun
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import { ElMessage } from 'element-plus'
import { ArrowLeft } from '@element-plus/icons-vue'

const route = useRoute()
const systemStore = useSystemStore()

// State
const selectedService = ref('')
const selectedTool = ref('')
const toolParams = ref({})
const executing = ref(false)
const executionResult = ref(null)
const paramsFormRef = ref()
const useJsonMode = ref(false)
const jsonInput = ref('')
const activeTab = ref('result')
const requestPayload = ref(null)
const historyRecords = ref([])

// Computed
const serviceNames = computed(() => {
  const names = new Set(systemStore.tools.map(tool => tool.service))
  return Array.from(names).sort()
})

const availableTools = computed(() => {
  if (!selectedService.value) return []
  return systemStore.tools.filter(tool => tool.service === selectedService.value)
})

const currentTool = computed(() => {
  if (!selectedTool.value) return null
  return systemStore.tools.find(tool => tool.name === selectedTool.value)
})

const toolParameters = computed(() => {
  if (!currentTool.value?.input_schema?.properties) return {}
  return currentTool.value.input_schema.properties
})

const hasParameters = computed(() => Object.keys(toolParameters.value).length > 0)

const paramRules = computed(() => {
  const rules = {}
  const required = currentTool.value?.input_schema?.required || []
  Object.keys(toolParameters.value).forEach(name => {
    if (required.includes(name)) {
      rules[name] = [{ required: true, message: 'Required', trigger: 'blur' }]
    }
  })
  return rules
})

const canExecute = computed(() => currentTool.value && !executing.value)

const formattedResult = computed(() => {
  if (!executionResult.value) return ''
  try {
     const data = executionResult.value.data
     if (typeof data === 'object') return JSON.stringify(data, null, 2)
     return data
  } catch {
     return executionResult.value.data
  }
})

// Methods
const isRequired = (name) => {
  return (currentTool.value?.input_schema?.required || []).includes(name)
}

const handleServiceChange = () => {
  selectedTool.value = ''
  toolParams.value = {}
  executionResult.value = null
}

const handleToolChange = () => {
  initializeParams()
  executionResult.value = null
  requestPayload.value = null
  jsonInput.value = hasParameters.value ? JSON.stringify(toolParams.value, null, 2) : '{}'
}

const initializeParams = () => {
  const params = {}
  Object.keys(toolParameters.value).forEach(name => {
    const p = toolParameters.value[name]
    if (p.type === 'array') params[name] = []
    else if (p.type === 'boolean') params[name] = false
    else if (p.default !== undefined) params[name] = p.default
    else params[name] = ''
  })
  toolParams.value = params
}

const resetParams = () => {
  initializeParams()
  executionResult.value = null
  jsonInput.value = hasParameters.value ? JSON.stringify(toolParams.value, null, 2) : '{}'
}

const loadExample = () => {
  Object.keys(toolParameters.value).forEach(name => {
    const p = toolParameters.value[name]
    if (p.example) toolParams.value[name] = p.example
    else if (p.type === 'string') toolParams.value[name] = `example_${name}`
    else if (p.type === 'integer') toolParams.value[name] = 10
    else if (p.type === 'boolean') toolParams.value[name] = true
  })
  jsonInput.value = JSON.stringify(toolParams.value, null, 2)
}

const executeTool = async () => {
  if (!currentTool.value) return
  
  try {
    if (hasParameters.value && !useJsonMode.value) {
      // Basic validation
    }
    
    executing.value = true
    let finalParams = {}
    
    if (useJsonMode.value) {
       try {
         finalParams = JSON.parse(jsonInput.value || '{}')
       } catch {
         ElMessage.error('Invalid JSON')
         return
       }
    } else {
       // Convert simplified form inputs
       Object.entries(toolParams.value).forEach(([k, v]) => {
          finalParams[k] = v
       })
    }
    
    requestPayload.value = finalParams
    const res = await systemStore.executeToolAction(selectedTool.value, finalParams)
    executionResult.value = res
    activeTab.value = 'result'
    
    historyRecords.value.unshift({
      tool_name: selectedTool.value,
      service_name: selectedService.value,
      response_time: res?.execution_info?.duration_ms,
      error: !res.success,
      params: finalParams
    })
    
  } catch (e) {
    ElMessage.error(e.message || 'Execution failed')
    executionResult.value = { success: false, data: e.message }
  } finally {
    executing.value = false
  }
}

const copyResult = () => {
  if(formattedResult.value) {
    navigator.clipboard.writeText(formattedResult.value)
    ElMessage.success('Copied')
  }
}

const formatJson = () => {
  try {
    const obj = JSON.parse(jsonInput.value)
    jsonInput.value = JSON.stringify(obj, null, 2)
  } catch (e) {
    ElMessage.warning('Invalid JSON')
  }
}

const pasteFromClipboard = async () => {
  try {
    jsonInput.value = await navigator.clipboard.readText()
  } catch (e) {
    ElMessage.warning('Clipboard access denied')
  }
}

const rerun = (rec) => {
  selectedService.value = rec.service_name
  selectedTool.value = rec.tool_name
  useJsonMode.value = true
  jsonInput.value = JSON.stringify(rec.params || {}, null, 2)
  executeTool()
}

// Watchers & Lifecycle
watch(() => [route.query.toolName, route.query.serviceName], ([t, s]) => {
  if (s) selectedService.value = s
  if (t) {
     // Wait for tools to load if needed
     setTimeout(() => {
        if(systemStore.tools.find(x => x.name === t)) {
           selectedTool.value = t
           handleToolChange()
        }
     }, 500)
  }
}, { immediate: true })

onMounted(async () => {
  await systemStore.fetchTools()
})
</script>

<style lang="scss" scoped>
.tool-execute-container {
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

// Main Layout
.main-layout {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 24px;
  
  @media (max-width: 900px) {
    grid-template-columns: 1fr;
  }
}

.panel-column {
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
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-secondary);
  }
}

.panel-controls {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  
  .mode-switch {
    cursor: pointer;
    color: var(--text-secondary);
    &.active { color: var(--text-primary); font-weight: 600; }
  }
  
  .divider { color: var(--border-color); }
  .spacer { width: 12px; }
}

.panel-body {
  background: var(--bg-surface);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: 20px;
}

// Form Elements
.form-row {
  display: flex;
  gap: 16px;
  margin-bottom: 16px;
  
  .form-group {
    flex: 1;
  }
}

.form-group label, .param-label {
  display: block;
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  color: var(--text-secondary);
  margin-bottom: 6px;
}

.atom-input {
  border: 1px solid var(--border-color);
  background: var(--bg-body);
  padding: 8px 12px;
  border-radius: 6px;
  font-size: 13px;
  color: var(--text-primary);
  width: 100%;
  box-sizing: border-box;
  font-family: var(--font-sans);
  
  &:focus { outline: none; border-color: var(--text-secondary); }
  &:disabled { background: var(--bg-hover); color: var(--text-disabled); cursor: not-allowed; }
}

select.atom-input {
  appearance: none;
  background-image: url("data:image/svg+xml;charset=UTF-8,%3csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3e%3cpolyline points='6 9 12 15 18 9'%3e%3c/polyline%3e%3c/svg%3e");
  background-repeat: no-repeat;
  background-position: right 8px center;
  background-size: 14px;
  padding-right: 30px;
}

.tool-summary {
  padding-top: 12px;
  border-top: 1px solid var(--border-color);
  .description {
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.5;
    margin: 0;
  }
}

// Parameters
.params-grid {
  display: grid;
  gap: 16px;
}

.param-field {
  .param-type {
    float: right;
    font-size: 10px;
    color: var(--text-placeholder);
    text-transform: lowercase;
  }
  
  .required { color: var(--color-danger); }
  
  .help-text {
    font-size: 11px;
    color: var(--text-secondary);
    margin-top: 4px;
  }
}

.checkbox-wrapper {
  display: flex;
  align-items: center;
  gap: 12px;
  height: 36px;
  
  .checkbox-label { font-size: 13px; color: var(--text-primary); }
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

.json-actions {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
  margin-top: 8px;
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

.empty-params {
  text-align: center;
  color: var(--text-placeholder);
  font-style: italic;
  font-size: 13px;
  padding: 20px;
}

// Right Column
.action-card {
  background: var(--bg-surface);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: 20px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  
  .action-info {
    display: flex;
    align-items: center;
    gap: 12px;
    
    .target-label {
      font-size: 14px;
      font-weight: 500;
      color: var(--text-primary);
    }
    
    .status-indicator {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: var(--border-color);
      &.ready { background: var(--color-success); }
    }
  }
  
  .execute-btn {
    min-width: 120px;
    font-weight: 600;
  }
}

// Output Section
.output-section {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 400px;
}

.tabs-header {
  display: flex;
  border-bottom: 1px solid var(--border-color);
  margin-bottom: 16px;
  
  .tab-item {
    padding: 8px 16px;
    font-size: 13px;
    font-weight: 500;
    color: var(--text-secondary);
    cursor: pointer;
    border-bottom: 2px solid transparent;
    transition: all 0.2s;
    
    &:hover { color: var(--text-primary); }
    &.active { color: var(--text-primary); border-bottom-color: var(--color-primary); }
  }
}

.tab-content {
  flex: 1;
  background: var(--bg-surface);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

// Result Views
.result-view, .request-view, .history-view {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.result-meta {
  display: flex;
  align-items: center;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-color);
  background: var(--bg-body);
  gap: 12px;
  
  .status-tag {
    font-size: 11px;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 4px;
    &.success { background: #dcfce7; color: #166534; }
    &.error { background: #fee2e2; color: #991b1b; }
  }
  
  .meta-item {
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--text-secondary);
  }
  
  .meta-actions {
    margin-left: auto;
  }
}

.result-body, .request-view {
  flex: 1;
  overflow: auto;
  padding: 16px;
  
  pre {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-primary);
  }
  
  .raw-output {
    white-space: pre-wrap;
    font-family: var(--font-mono);
    font-size: 12px;
  }
}

.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--text-placeholder);
  font-size: 13px;
  font-style: italic;
}

// History
.history-list {
  display: flex;
  flex-direction: column;
  overflow: auto;
}

.history-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-color);
  &:last-child { border-bottom: none; }
  
  .history-main {
    display: flex;
    flex-direction: column;
    gap: 2px;
    
    .hist-name { font-size: 13px; font-weight: 500; color: var(--text-primary); }
    .hist-time { font-size: 11px; color: var(--text-secondary); font-family: var(--font-mono); }
  }
  
  .history-actions {
    display: flex;
    align-items: center;
    gap: 12px;
    
    .status-dot {
      width: 6px;
      height: 6px;
      border-radius: 50%;
      &.success { background: var(--color-success); }
      &.error { background: var(--color-danger); }
    }
  }
}
</style>
