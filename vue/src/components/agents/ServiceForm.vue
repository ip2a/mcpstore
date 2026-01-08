<template>
  <div class="agent-service-add" :class="{ compact }">
    <div class="main-layout">
      <div class="panel-column left-col">
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
                :class="{ active: serviceForm.serviceType === 'remote' }"
                @click="setServiceType('remote')"
              >
                <span class="type-icon"><Link /></span>
                <span class="type-label">Remote (HTTP/SSE)</span>
              </div>
              <div
                class="type-option"
                :class="{ active: serviceForm.serviceType === 'local' }"
                @click="setServiceType('local')"
              >
                <span class="type-icon"><FolderOpened /></span>
                <span class="type-label">Local (Stdio)</span>
              </div>
            </div>
          </div>
        </section>

        <section class="panel-section">
          <div class="panel-header">
            <h3 class="panel-title">
              Agent Context
            </h3>
          </div>
          <div class="panel-body form-card">
            <div class="form-content">
              <div class="form-group">
                <label>Agent ID</label>
                <input
                  v-model="serviceForm.agentId"
                  class="atom-input"
                  placeholder="可选：绑定到指定 Agent（留空则添加到 Store）"
                  :disabled="!!defaultAgentId"
                >
                <p class="form-tip">
                  用于命名空间隔离，推荐仅使用字母、数字、`_`、`-`；留空时直接在 Store 维度注册。
                </p>
              </div>
              <div class="form-row">
                <div class="form-group">
                  <label>Service Name <span class="required">*</span></label>
                  <input
                    v-model="serviceForm.name"
                    class="atom-input"
                    placeholder="weather, files, sync..."
                  >
                </div>
                <div class="form-group">
                  <label>Description</label>
                  <input
                    v-model="serviceForm.description"
                    class="atom-input"
                    placeholder="可选：记录用途或能力"
                  >
                </div>
              </div>
            </div>
          </div>
        </section>

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
            <div class="form-content">
              <div
                v-if="serviceForm.serviceType === 'remote'"
                class="form-stack"
              >
                <div class="form-row">
                  <div class="form-group">
                    <label>Service URL <span class="required">*</span></label>
                    <input
                      v-model="serviceForm.url"
                      class="atom-input"
                      placeholder="https://example.com/mcp"
                    >
                  </div>
                  <div class="form-group">
                    <label>Transport</label>
                    <select
                      v-model="serviceForm.transport"
                      class="atom-input"
                    >
                      <option value="streamable-http">
                        Streamable HTTP
                      </option>
                      <option value="sse">
                        SSE
                      </option>
                    </select>
                  </div>
                </div>
              </div>

              <div
                v-if="serviceForm.serviceType === 'local'"
                class="form-stack"
              >
                <div class="form-row">
                  <div class="form-group">
                    <label>Command <span class="required">*</span></label>
                    <input
                      v-model="serviceForm.command"
                      class="atom-input"
                      placeholder="python, npx, bun..."
                    >
                  </div>
                  <div class="form-group">
                    <label>Working Directory</label>
                    <input
                      v-model="serviceForm.working_dir"
                      class="atom-input"
                      placeholder="可选路径"
                    >
                  </div>
                </div>

                <div class="form-group">
                  <label class="group-label">Arguments</label>
                  <textarea
                    v-model="argsText"
                    class="code-editor"
                    rows="4"
                    placeholder="每行一个参数，回车换行"
                    @input="updateArgs"
                  />
                  <p class="form-tip">
                    例如：<code>./server.py</code> 回车 <code>--port</code> 回车 <code>8080</code>
                  </p>
                </div>

                <div class="form-group">
                  <label class="group-label">Environment Variables</label>
                  <div
                    v-for="(envVar, index) in envVars"
                    :key="index"
                    class="dynamic-row"
                  >
                    <input
                      v-model="envVar.key"
                      class="atom-input key"
                      placeholder="KEY"
                    >
                    <input
                      v-model="envVar.value"
                      class="atom-input value"
                      placeholder="VALUE"
                    >
                    <button
                      class="icon-btn danger"
                      type="button"
                      @click="removeEnvVar(index)"
                    >
                      <el-icon><Delete /></el-icon>
                    </button>
                  </div>
                  <button
                    class="text-btn primary"
                    type="button"
                    @click="addEnvVar"
                  >
                    + 添加环境变量
                  </button>
                </div>
              </div>
            </div>

            <div class="submit-row">
              <el-button
                type="primary"
                color="#000"
                size="large"
                :loading="adding"
                class="submit-btn"
                @click="addService"
              >
                {{ adding ? '添加中...' : '提交' }}
              </el-button>
            </div>
          </div>
        </section>
      </div>

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
                <label>AGENT</label>
                <span class="val">{{ previewService.agentId || 'Store (无 Agent)' }}</span>
              </div>
              <div class="preview-item">
                <label>SERVICE</label>
                <span class="val">{{ previewService.name }}</span>
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
                <div class="preview-item">
                  <label>TRANSPORT</label>
                  <span class="val">{{ previewService.transport }}</span>
                </div>
              </template>

              <template v-else>
                <div class="preview-item">
                  <label>COMMAND</label>
                  <span class="code">$ {{ previewService.command || '-' }}</span>
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
                <label>PAYLOAD</label>
                <pre>{{ configPreview }}</pre>
              </div>
            </div>
            <div
              v-else
              class="empty-state"
            >
              填写表单后可预览提交内容
            </div>
          </div>
        </section>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, computed, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { Link, FolderOpened, Delete } from '@element-plus/icons-vue'
import { useAgentsStore } from '@/stores/agents'
import { api } from '@/api'

const props = defineProps({
  defaultAgentId: {
    type: String,
    default: ''
  },
  compact: {
    type: Boolean,
    default: false
  }
})

const emit = defineEmits(['success', 'cancel'])
const agentsStore = useAgentsStore()

const createDefaultForm = () => ({
  agentId: props.defaultAgentId || '',
  serviceType: 'remote',
  name: '',
  url: '',
  transport: 'streamable-http',
  command: '',
  args: [],
  working_dir: '',
  env: {},
  description: ''
})

const validateService = (serviceForm) => {
  const errors = []

  if (serviceForm.agentId && !/^[a-zA-Z0-9_-]+$/.test(serviceForm.agentId)) {
    errors.push('Agent ID只能包含字母、数字、下划线和连字符')
  }
  
  if (!serviceForm.name || serviceForm.name.trim() === '') {
    errors.push('服务名称不能为空')
  } else if (!/^[a-zA-Z0-9_-]+$/.test(serviceForm.name)) {
    errors.push('服务名称只能包含字母、数字、下划线和连字符')
  }
  
  if (serviceForm.serviceType === 'remote') {
    if (!serviceForm.url || serviceForm.url.trim() === '') {
      errors.push('远程服务URL不能为空')
    }
    try {
      if (serviceForm.url) new URL(serviceForm.url)
    } catch {
      errors.push('请输入有效的URL')
    }
  } else {
    if (!serviceForm.command || serviceForm.command.trim() === '') {
      errors.push('本地服务命令不能为空')
    }
  }
  
  return {
    isValid: errors.length === 0,
    errors
  }
}

const adding = ref(false)
const argsText = ref('')
const envVars = ref([{ key: '', value: '' }])
const serviceForm = ref(createDefaultForm())

const onServiceTypeChange = () => {
  if (serviceForm.value.serviceType === 'remote') {
    serviceForm.value.command = ''
    serviceForm.value.args = []
    serviceForm.value.working_dir = ''
    serviceForm.value.env = {}
    argsText.value = ''
  } else {
    serviceForm.value.url = ''
    serviceForm.value.transport = 'streamable-http'
    argsText.value = serviceForm.value.args.join('\n')
  }
}

const setServiceType = (type) => {
  if (serviceForm.value.serviceType === type) return
  serviceForm.value.serviceType = type
  onServiceTypeChange()
}

const updateArgs = () => {
  serviceForm.value.args = argsText.value
    .split('\n')
    .map(arg => arg.trim())
    .filter(arg => arg.length > 0)
}

const addEnvVar = () => {
  envVars.value.push({ key: '', value: '' })
}

const removeEnvVar = (index) => {
  if (envVars.value.length > 1) {
    envVars.value.splice(index, 1)
  }
  updateEnvObject()
}

const updateEnvObject = () => {
  serviceForm.value.env = {}
  envVars.value.forEach(envVar => {
    if (envVar.key && envVar.value) {
      serviceForm.value.env[envVar.key] = envVar.value
    }
  })
}

const createServiceConfig = () => {
  const serviceConfig = {
    name: serviceForm.value.name,
    description: serviceForm.value.description
  }

  if (serviceForm.value.serviceType === 'remote') {
    serviceConfig.url = serviceForm.value.url
    serviceConfig.transport = serviceForm.value.transport
  } else {
    serviceConfig.command = serviceForm.value.command
    serviceConfig.args = serviceForm.value.args
    if (serviceForm.value.working_dir) {
      serviceConfig.working_dir = serviceForm.value.working_dir
    }
    if (Object.keys(serviceForm.value.env).length > 0) {
      serviceConfig.env = serviceForm.value.env
    }
  }

  return serviceConfig
}

const previewService = computed(() => {
  if (!serviceForm.value.name) return null
  const base = {
    agentId: serviceForm.value.agentId,
    name: serviceForm.value.name,
    type: serviceForm.value.serviceType
  }

  if (serviceForm.value.serviceType === 'remote') {
    return {
      ...base,
      url: serviceForm.value.url,
      transport: serviceForm.value.transport
    }
  }

  return {
    ...base,
    command: serviceForm.value.command,
    args: serviceForm.value.args
  }
})

const configPreview = computed(() => {
  if (!serviceForm.value.name) return ''
  try {
    const payload = createServiceConfig()
    if (serviceForm.value.agentId) {
      return JSON.stringify({
        agent_id: serviceForm.value.agentId,
        service: payload
      }, null, 2)
    }
    return JSON.stringify(payload, null, 2)
  } catch {
    return ''
  }
})

const addService = async () => {
  try {
    updateEnvObject()
    
    const validation = validateService(serviceForm.value)
    if (!validation.isValid) {
      ElMessage.error(validation.errors[0])
      return
    }
    
    adding.value = true
    
    const serviceConfig = createServiceConfig()

    if (serviceForm.value.agentId) {
      const result = await agentsStore.addService(serviceForm.value.agentId, serviceConfig)
      if (result.success) {
        ElMessage.success('服务添加成功（Agent）')
        emit('success', { scope: 'agent', agentId: serviceForm.value.agentId, service: serviceConfig })
      } else {
        ElMessage.error('服务添加失败: ' + result.error)
      }
    } else {
      const res = await api.store.addService(serviceConfig)
      if (res.data?.success !== false) {
        ElMessage.success('服务添加成功（Store）')
        emit('success', { scope: 'store', service: serviceConfig })
      } else {
        ElMessage.error('服务添加失败: ' + (res.data?.message || '未知错误'))
      }
    }
  } catch (error) {
    if (error !== 'validation failed') {
      ElMessage.error('服务添加失败: ' + (error.message || error))
    }
  } finally {
    adding.value = false
  }
}

const resetForm = () => {
  serviceForm.value = createDefaultForm()
  argsText.value = ''
  envVars.value = [{ key: '', value: '' }]
}

onMounted(() => {
  if (props.defaultAgentId) {
    serviceForm.value.agentId = props.defaultAgentId
  }
})

watch(() => props.defaultAgentId, (val) => {
  if (val && val !== serviceForm.value.agentId) {
    serviceForm.value.agentId = val
  }
})
</script>

<style lang="scss" scoped>
.agent-service-add {
  width: 100%;
}

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

.type-selector-card {
  padding: 20px;
  background: var(--bg-surface);
  border: 1px solid var(--border-color);
  border-radius: 8px;
}

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
  @media (max-width: 700px) {
    flex-direction: column;
  }
}

.form-group {
  label {
    display: block;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    color: var(--text-secondary);
    margin-bottom: 6px;
    
    .required {
      color: var(--color-danger);
    }
  }

  .group-label {
    margin-top: 12px;
    margin-bottom: 8px;
  }
}

.form-tip {
  font-size: 11px;
  color: var(--text-secondary);
  margin-top: 6px;
  line-height: 1.4;
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
  
  &.key { width: 40%; }
  &.value { flex: 1; }
}

select.atom-input {
  appearance: none;
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
