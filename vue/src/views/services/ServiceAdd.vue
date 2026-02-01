<template>
  <div class="page-shell service-add-container">
    <!-- Header -->
    <header class="page-header">
      <div class="header-content">
        <h1 class="page-title">
          Add Service
        </h1>
        <p class="page-subtitle">
          Register a new MCP service instance
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
      <!-- Left Panel: Configuration Form -->
      <div class="panel-column left-col">
        <!-- Service Type Selection -->
        <section class="panel-section">
          <div class="panel-header">
            <h3 class="panel-title">
              Service Type
            </h3>
          </div>
          <div class="panel-body type-selector-card">
            <div class="type-options">
              <div 
                class="type-option" 
                :class="{ active: serviceType === 'remote' }"
                @click="serviceType = 'remote'"
              >
                <span class="type-icon"><Link /></span>
                <span class="type-label">Remote (HTTP/SSE)</span>
              </div>
              <div 
                class="type-option" 
                :class="{ active: serviceType === 'local' }"
                @click="serviceType = 'local'"
              >
                <span class="type-icon"><FolderOpened /></span>
                <span class="type-label">Local (Stdio)</span>
              </div>
              <div 
                class="type-option" 
                :class="{ active: serviceType === 'mcpServers' }"
                @click="serviceType = 'mcpServers'"
              >
                <span class="type-icon"><DocumentCopy /></span>
                <span class="type-label">Config (JSON)</span>
              </div>
            </div>
          </div>
        </section>

        <!-- Configuration Form -->
        <section class="panel-section">
          <div class="panel-header">
            <h3 class="panel-title">
              Configuration
            </h3>
            <div class="panel-controls">
              <el-button
                link
                size="small"
                @click="resetForm"
              >
                Reset
              </el-button>
            </div>
          </div>
          
          <div class="panel-body form-card">
            <!-- Remote Service Form -->
            <div
              v-if="serviceType === 'remote'"
              class="form-content"
            >
              <div class="form-row">
                <div class="form-group">
                  <label>Service Name <span class="required">*</span></label>
                  <input
                    v-model="remoteForm.name"
                    class="atom-input"
                    placeholder="e.g. weather-service"
                  >
                </div>
                <div class="form-group">
                  <label>Transport</label>
                  <select
                    v-model="remoteForm.transport"
                    class="atom-input"
                  >
                    <option value="">
                      Auto Detect
                    </option>
                    <option value="streamable-http">
                      HTTP Stream
                    </option>
                    <option value="sse">
                      SSE
                    </option>
                  </select>
                </div>
              </div>
               
              <div class="form-group">
                <label>Service URL <span class="required">*</span></label>
                <input
                  v-model="remoteForm.url"
                  class="atom-input"
                  placeholder="https://..."
                >
              </div>
               
              <!-- Headers -->
              <div class="form-group">
                <label class="group-label">Headers</label>
                <div
                  v-for="(h, idx) in remoteForm.headers"
                  :key="idx"
                  class="dynamic-row"
                >
                  <input
                    v-model="h.key"
                    class="atom-input key"
                    placeholder="Header Name"
                  >
                  <input
                    v-model="h.value"
                    class="atom-input value"
                    placeholder="Value"
                  >
                  <button
                    class="icon-btn danger"
                    @click="removeHeader(idx)"
                  >
                    <el-icon><Delete /></el-icon>
                  </button>
                </div>
                <button
                  class="text-btn primary"
                  @click="addHeader"
                >
                  + Add Header
                </button>
              </div>
               
              <!-- Environment -->
              <div class="form-group">
                <label class="group-label">Environment Variables</label>
                <div
                  v-for="(e, idx) in remoteForm.env"
                  :key="idx"
                  class="dynamic-row"
                >
                  <input
                    v-model="e.key"
                    class="atom-input key"
                    placeholder="KEY"
                  >
                  <input
                    v-model="e.value"
                    class="atom-input value"
                    placeholder="VALUE"
                  >
                  <button
                    class="icon-btn danger"
                    @click="removeEnv(idx)"
                  >
                    <el-icon><Delete /></el-icon>
                  </button>
                </div>
                <button
                  class="text-btn primary"
                  @click="addEnv"
                >
                  + Add Env Var
                </button>
              </div>
            </div>

            <!-- Local Service Form -->
            <div
              v-if="serviceType === 'local'"
              class="form-content"
            >
              <div class="form-row">
                <div class="form-group">
                  <label>Service Name <span class="required">*</span></label>
                  <input
                    v-model="localForm.name"
                    class="atom-input"
                    placeholder="e.g. file-system"
                  >
                </div>
                <div class="form-group">
                  <label>Command <span class="required">*</span></label>
                  <input
                    v-model="localForm.command"
                    class="atom-input"
                    placeholder="e.g. npx, python"
                  >
                </div>
              </div>
               
              <div class="form-group">
                <label>Working Directory</label>
                <input
                  v-model="localForm.working_dir"
                  class="atom-input"
                  placeholder="Optional path"
                >
              </div>
               
              <!-- Args -->
              <div class="form-group">
                <label class="group-label">Arguments</label>
                <div
                  v-for="(arg, idx) in localForm.args"
                  :key="idx"
                  class="dynamic-row"
                >
                  <input
                    v-model="localForm.args[idx]"
                    class="atom-input full"
                    placeholder="Argument"
                  >
                  <button
                    class="icon-btn danger"
                    @click="removeArg(idx)"
                  >
                    <el-icon><Delete /></el-icon>
                  </button>
                </div>
                <button
                  class="text-btn primary"
                  @click="addArg"
                >
                  + Add Argument
                </button>
              </div>
               
              <!-- Environment -->
              <div class="form-group">
                <label class="group-label">Environment Variables</label>
                <div
                  v-for="(e, idx) in localForm.env"
                  :key="idx"
                  class="dynamic-row"
                >
                  <input
                    v-model="e.key"
                    class="atom-input key"
                    placeholder="KEY"
                  >
                  <input
                    v-model="e.value"
                    class="atom-input value"
                    placeholder="VALUE"
                  >
                  <button
                    class="icon-btn danger"
                    @click="removeLocalEnv(idx)"
                  >
                    <el-icon><Delete /></el-icon>
                  </button>
                </div>
                <button
                  class="text-btn primary"
                  @click="addLocalEnv"
                >
                  + Add Env Var
                </button>
              </div>
            </div>

            <!-- JSON Config Form -->
            <div
              v-if="serviceType === 'mcpServers'"
              class="form-content"
            >
              <div class="form-group full-height">
                <label>JSON Configuration <span class="required">*</span></label>
                <textarea 
                  v-model="mcpServersForm.content" 
                  class="code-editor" 
                  rows="12"
                  placeholder="{ &quot;mcpServers&quot;: { ... } }"
                />
              </div>
              <div class="form-actions">
                <button
                  class="text-btn"
                  @click="formatJson"
                >
                  Format
                </button>
                <button
                  class="text-btn"
                  @click="loadExample"
                >
                  Load Example
                </button>
              </div>
            </div>

            <!-- Submit Action -->
            <div class="submit-row">
              <el-button 
                type="primary" 
                color="#000" 
                size="large" 
                :loading="submitting"
                class="submit-btn"
                @click="submitForm"
              >
                Register Service
              </el-button>
            </div>
          </div>
        </section>
      </div>

      <!-- Right Panel: Preview -->
      <div class="panel-column right-col">
        <section class="panel-section full-height">
          <div class="panel-header">
            <h3 class="panel-title">
              Preview
            </h3>
          </div>
          <div class="panel-body preview-card">
            <div
              v-if="previewService"
              class="preview-content"
            >
              <div class="preview-item">
                <label>NAME</label>
                <span class="val">{{ previewService.name || '-' }}</span>
              </div>
              <div class="preview-item">
                <label>TYPE</label>
                <span class="tag">{{ previewService.type }}</span>
              </div>
                
              <template v-if="previewService.type === 'remote'">
                <div class="preview-item">
                  <label>URL</label>
                  <span class="code">{{ previewService.url || '-' }}</span>
                </div>
                <div
                  v-if="previewService.transport"
                  class="preview-item"
                >
                  <label>TRANSPORT</label>
                  <span class="val">{{ previewService.transport }}</span>
                </div>
              </template>
                
              <template v-if="previewService.type === 'local'">
                <div class="preview-item">
                  <label>COMMAND</label>
                  <span class="code">$ {{ previewService.command }}</span>
                </div>
                <div
                  v-if="previewService.args?.length"
                  class="preview-item"
                >
                  <label>ARGS</label>
                  <span class="code">{{ previewService.args.join(' ') }}</span>
                </div>
              </template>

              <div class="preview-divider" />
                
              <div class="preview-json">
                <label>GENERATED CONFIG</label>
                <pre>{{ configPreview }}</pre>
              </div>
            </div>
             
            <div
              v-else
              class="empty-state"
            >
              Fill out the form to see preview.
            </div>
          </div>
        </section>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, reactive } from 'vue'
import { useRouter } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import { ElMessage } from 'element-plus'
import {
  Link, FolderOpened, DocumentCopy, Delete, ArrowLeft
} from '@element-plus/icons-vue'

const router = useRouter()
const systemStore = useSystemStore()

const serviceType = ref('remote')
const submitting = ref(false)

const remoteForm = reactive({ name: '', url: '', transport: '', headers: [], env: [] })
const localForm = reactive({ name: '', command: '', args: [], working_dir: '', env: [] })
const mcpServersForm = reactive({ content: '' })

// Computed Preview
const previewService = computed(() => {
  try {
    let s = { type: serviceType.value }
    if (serviceType.value === 'remote') {
      s.name = remoteForm.name
      s.url = remoteForm.url
      s.transport = remoteForm.transport
    } else if (serviceType.value === 'local') {
      s.name = localForm.name
      s.command = localForm.command
      s.args = localForm.args.filter(Boolean)
    } else {
       try {
         const p = JSON.parse(mcpServersForm.content)
         const k = Object.keys(p.mcpServers || {})[0]
         if(k) {
            s.name = k
            const c = p.mcpServers[k]
            s.type = c.url ? 'remote' : 'local'
            Object.assign(s, c)
         }
       } catch { return null }
    }
    return s.name ? s : null
  } catch { return null }
})

const configPreview = computed(() => {
   try {
     let c = {}
     if(serviceType.value === 'remote') {
        c = { name: remoteForm.name, url: remoteForm.url }
        if(remoteForm.transport) c.transport = remoteForm.transport
        if(remoteForm.headers.length) c.headers = Object.fromEntries(remoteForm.headers.map(h=>[h.key, h.value]))
        if(remoteForm.env.length) c.env = Object.fromEntries(remoteForm.env.map(e=>[e.key, e.value]))
     } else if(serviceType.value === 'local') {
        c = { name: localForm.name, command: localForm.command, args: localForm.args.filter(Boolean) }
        if(localForm.working_dir) c.working_dir = localForm.working_dir
        if(localForm.env.length) c.env = Object.fromEntries(localForm.env.map(e=>[e.key, e.value]))
     } else {
        return mcpServersForm.content
     }
     return JSON.stringify(c, null, 2)
   } catch { return '' }
})

// Methods
const addHeader = () => remoteForm.headers.push({ key: '', value: '' })
const removeHeader = (i) => remoteForm.headers.splice(i, 1)
const addEnv = () => remoteForm.env.push({ key: '', value: '' })
const removeEnv = (i) => remoteForm.env.splice(i, 1)
const addArg = () => localForm.args.push('')
const removeArg = (i) => localForm.args.splice(i, 1)
const addLocalEnv = () => localForm.env.push({ key: '', value: '' })
const removeLocalEnv = (i) => localForm.env.splice(i, 1)

const formatJson = () => {
  try {
    const p = JSON.parse(mcpServersForm.content)
    mcpServersForm.content = JSON.stringify(p, null, 2)
  } catch (e) {
    ElMessage.warning('Invalid JSON')
  }
}

const loadExample = () => {
   mcpServersForm.content = JSON.stringify({
     mcpServers: {
       'example-service': {
         'command': 'npx',
         'args': ['-y', '@modelcontextprotocol/server-filesystem', '.']
       }
     }
   }, null, 2)
}

const resetForm = () => {
  if(serviceType.value==='remote') Object.assign(remoteForm, {name:'', url:'', transport:'', headers:[], env:[]})
  else if(serviceType.value==='local') Object.assign(localForm, {name:'', command:'', args:[], working_dir:'', env:[]})
  else mcpServersForm.content = ''
}

const submitForm = async () => {
   if(!previewService.value) return ElMessage.warning('Invalid configuration')
   
   try {
     submitting.value = true
     const config = JSON.parse(configPreview.value)
     await systemStore.addService(config)
     ElMessage.success('Service added')
     router.push('/for_store/list_services')
   } catch (e) {
     ElMessage.error(e.message || 'Failed to add service')
   } finally {
     submitting.value = false
   }
}
</script>

<style lang="scss" scoped>
.service-add-container {
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

// Layout
.main-layout {
  display: grid;
  grid-template-columns: 1.5fr 1fr;
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

// Type Selector
.type-options {
  display: flex;
  gap: 12px;
  
  .type-option {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 16px;
    border: 1px solid var(--border-color);
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.2s;
    background: var(--bg-surface);
    
    .type-icon {
      font-size: 20px;
      margin-bottom: 8px;
      color: var(--text-secondary);
    }
    
    .type-label {
      font-size: 12px;
      font-weight: 500;
      color: var(--text-secondary);
    }
    
    &:hover { border-color: var(--text-primary); }
    &.active { 
       border-color: var(--text-primary); 
       background: var(--bg-hover);
       .type-icon, .type-label { color: var(--text-primary); }
    }
  }
}

// Form Card
.form-card {
  background: var(--bg-surface);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 24px;
}

.form-content {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.form-row {
  display: flex;
  gap: 16px;
  .form-group { flex: 1; }
}

.form-group {
  margin-bottom: 4px;
  label {
    display: block;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    color: var(--text-secondary);
    margin-bottom: 6px;
    
    .required { color: var(--color-danger); }
  }
  
  .group-label {
    margin-top: 12px;
    margin-bottom: 8px;
  }
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
  
  &.full { width: 100%; }
  &.key { width: 40%; }
  &.value { flex: 1; }
}

select.atom-input {
  appearance: none;
}

.dynamic-row {
  display: flex;
  gap: 8px;
  margin-bottom: 8px;
}

.icon-btn {
  background: none;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  cursor: pointer;
  padding: 4px 8px;
  color: var(--text-secondary);
  
  &:hover { color: var(--text-primary); border-color: var(--text-primary); }
  &.danger:hover { color: var(--color-danger); border-color: var(--color-danger); }
}

.text-btn {
  background: none;
  border: none;
  cursor: pointer;
  font-size: 12px;
  color: var(--text-secondary);
  padding: 0;
  &:hover { color: var(--text-primary); text-decoration: underline; }
  &.primary { color: var(--text-primary); font-weight: 500; }
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

.form-actions {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
  margin-top: 8px;
}

.submit-row {
  margin-top: 24px;
  padding-top: 16px;
  border-top: 1px solid var(--border-color);
  display: flex;
  justify-content: flex-end;
  
  .submit-btn {
    min-width: 160px;
    font-weight: 600;
  }
}

// Preview
.preview-card {
  background: var(--bg-surface);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 20px;
  height: 100%;
}

.preview-content {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.preview-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
  
  label {
    font-size: 10px;
    font-weight: 700;
    color: var(--text-secondary);
  }
  
  .val { font-size: 13px; color: var(--text-primary); word-break: break-all; }
  .code { font-family: var(--font-mono); font-size: 12px; color: var(--text-primary); background: var(--bg-body); padding: 4px; border-radius: 4px; word-break: break-all; }
  .tag { 
    display: inline-block; 
    font-size: 11px; 
    padding: 2px 6px; 
    background: var(--bg-hover); 
    border-radius: 4px; 
    color: var(--text-primary); 
    width: fit-content;
  }
}

.preview-divider {
  height: 1px;
  background: var(--border-color);
  margin: 8px 0;
}

.preview-json {
  label {
    font-size: 10px;
    font-weight: 700;
    color: var(--text-secondary);
    display: block;
    margin-bottom: 8px;
  }
  pre {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
    overflow: auto;
  }
}

.empty-state {
  color: var(--text-placeholder);
  font-size: 13px;
  font-style: italic;
  text-align: center;
  margin-top: 40px;
}
</style>
