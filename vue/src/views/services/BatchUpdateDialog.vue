<template>
  <el-dialog
    v-model="visible"
    title="批量更新服务"
    width="800px"
    :close-on-click-modal="false"
    @close="handleClose"
  >
    <div class="batch-update-dialog">
      <!-- 选中的服务列表 -->
      <div class="selected-services">
        <h4>选中的服务 ({{ services.length }}个)</h4>
        <div class="service-tags">
          <el-tag
            v-for="service in services"
            :key="service.name"
            class="service-tag"
          >
            {{ service.name }}
          </el-tag>
        </div>
      </div>

      <!-- 更新选项 -->
      <div class="update-options">
        <h4>更新选项</h4>
        
        <el-form
          ref="formRef"
          :model="form"
          label-width="120px"
        >
          <!-- 传输类型 -->
          <el-form-item label="传输类型">
            <el-checkbox v-model="form.updateTransport">更新传输类型</el-checkbox>
            <el-select
              v-model="form.transport"
              :disabled="!form.updateTransport"
              placeholder="选择传输类型"
              style="margin-left: 12px; width: 200px;"
            >
              <el-option label="HTTP" value="http" />
              <el-option label="Streamable HTTP" value="streamable-http" />
              <el-option label="SSE" value="sse" />
            </el-select>
          </el-form-item>

          <!-- 超时时间 -->
          <el-form-item label="超时时间">
            <el-checkbox v-model="form.updateTimeout">更新超时时间</el-checkbox>
            <el-input-number
              v-model="form.timeout"
              :disabled="!form.updateTimeout"
              :min="1"
              :max="300"
              style="margin-left: 12px; width: 200px;"
            />
            <span style="margin-left: 8px; color: var(--text-secondary);">秒</span>
          </el-form-item>

          <!-- 保持连接 -->
          <el-form-item label="保持连接">
            <el-checkbox v-model="form.updateKeepAlive">更新保持连接</el-checkbox>
            <el-switch
              v-model="form.keep_alive"
              :disabled="!form.updateKeepAlive"
              style="margin-left: 12px;"
            />
          </el-form-item>

          <!-- 请求头 -->
          <el-form-item label="请求头">
            <el-checkbox v-model="form.updateHeaders">更新请求头</el-checkbox>
            <div v-if="form.updateHeaders" class="headers-editor">
              <div
                v-for="(header, index) in form.headers"
                :key="index"
                class="header-row"
              >
                <el-input
                  v-model="header.key"
                  placeholder="Header名称"
                  style="width: 150px;"
                />
                <el-input
                  v-model="header.value"
                  placeholder="Header值"
                  style="width: 200px; margin-left: 8px;"
                />
                <el-button
                  type="danger"
                  :icon="Delete"
                  @click="removeHeader(index)"
                  style="margin-left: 8px;"
                />
              </div>
              <el-button
                type="primary"
                :icon="Plus"
                @click="addHeader"
                style="margin-top: 8px;"
              >
                添加Header
              </el-button>
            </div>
          </el-form-item>

          <!-- 环境变量 -->
          <el-form-item label="环境变量">
            <el-checkbox v-model="form.updateEnv">更新环境变量</el-checkbox>
            <div v-if="form.updateEnv" class="env-editor">
              <div
                v-for="(env, index) in form.env"
                :key="index"
                class="env-row"
              >
                <el-input
                  v-model="env.key"
                  placeholder="变量名"
                  style="width: 150px;"
                />
                <el-input
                  v-model="env.value"
                  placeholder="变量值"
                  style="width: 200px; margin-left: 8px;"
                  :type="env.key.toLowerCase().includes('password') || env.key.toLowerCase().includes('key') ? 'password' : 'text'"
                  show-password
                />
                <el-button
                  type="danger"
                  :icon="Delete"
                  @click="removeEnv(index)"
                  style="margin-left: 8px;"
                />
              </div>
              <el-button
                type="primary"
                :icon="Plus"
                @click="addEnv"
                style="margin-top: 8px;"
              >
                添加环境变量
              </el-button>
            </div>
          </el-form-item>
        </el-form>
      </div>

      <!-- 预览更新 -->
      <div class="update-preview">
        <h4>更新预览</h4>
        <el-input
          v-model="updatePreview"
          type="textarea"
          :rows="8"
          readonly
          class="preview-text"
        />
      </div>
    </div>

    <template #footer>
      <el-button @click="handleClose">取消</el-button>
      <el-button
        type="primary"
        @click="handleUpdate"
        :loading="updating"
        :disabled="!hasUpdates"
      >
        批量更新
      </el-button>
    </template>
  </el-dialog>
</template>

<script setup>
import { ref, computed, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { api } from '@/api'
import { Plus, Delete } from '@element-plus/icons-vue'

const props = defineProps({
  modelValue: {
    type: Boolean,
    default: false
  },
  services: {
    type: Array,
    default: () => []
  }
})

const emit = defineEmits(['update:modelValue', 'updated'])

// 响应式数据
const formRef = ref()
const updating = ref(false)

const visible = computed({
  get: () => props.modelValue,
  set: (value) => emit('update:modelValue', value)
})

const form = ref({
  updateTransport: false,
  transport: 'http',
  updateTimeout: false,
  timeout: 30,
  updateKeepAlive: false,
  keep_alive: true,
  updateHeaders: false,
  headers: [],
  updateEnv: false,
  env: []
})

// 计算属性
const hasUpdates = computed(() => {
  return form.value.updateTransport ||
         form.value.updateTimeout ||
         form.value.updateKeepAlive ||
         form.value.updateHeaders ||
         form.value.updateEnv
})

const updatePreview = computed(() => {
  const updates = {}
  
  if (form.value.updateTransport) {
    updates.transport = form.value.transport
  }
  
  if (form.value.updateTimeout) {
    updates.timeout = form.value.timeout
  }
  
  if (form.value.updateKeepAlive) {
    updates.keep_alive = form.value.keep_alive
  }
  
  if (form.value.updateHeaders) {
    const headers = {}
    form.value.headers.forEach(header => {
      if (header.key && header.value) {
        headers[header.key] = header.value
      }
    })
    if (Object.keys(headers).length > 0) {
      updates.headers = headers
    }
  }
  
  if (form.value.updateEnv) {
    const env = {}
    form.value.env.forEach(envVar => {
      if (envVar.key && envVar.value) {
        env[envVar.key] = envVar.value
      }
    })
    if (Object.keys(env).length > 0) {
      updates.env = env
    }
  }
  
  return JSON.stringify(updates, null, 2)
})

// 方法
const addHeader = () => {
  form.value.headers.push({ key: '', value: '' })
}

const removeHeader = (index) => {
  form.value.headers.splice(index, 1)
}

const addEnv = () => {
  form.value.env.push({ key: '', value: '' })
}

const removeEnv = (index) => {
  form.value.env.splice(index, 1)
}

const handleUpdate = async () => {
  if (!hasUpdates.value) {
    ElMessage.warning('请选择要更新的选项')
    return
  }
  
  try {
    updating.value = true
    
    // 构建更新数据
    const updates = props.services.map(service => ({
      name: service.name,
      config: JSON.parse(updatePreview.value)
    }))
    
    const response = await api.store.batchUpdateServices(updates)
    
    if (response.data.success) {
      ElMessage.success('批量更新成功')
      emit('updated')
      handleClose()
    } else {
      ElMessage.error(response.data.message || '批量更新失败')
    }
  } catch (error) {
    ElMessage.error('批量更新失败')
  } finally {
    updating.value = false
  }
}

const handleClose = () => {
  visible.value = false
  resetForm()
}

const resetForm = () => {
  form.value = {
    updateTransport: false,
    transport: 'http',
    updateTimeout: false,
    timeout: 30,
    updateKeepAlive: false,
    keep_alive: true,
    updateHeaders: false,
    headers: [],
    updateEnv: false,
    env: []
  }
}

// 监听对话框打开
watch(visible, (newVal) => {
  if (newVal) {
    resetForm()
  }
})
</script>

<style lang="scss" scoped>
.batch-update-dialog {
  .selected-services {
    margin-bottom: 24px;
    
    h4 {
      margin: 0 0 12px 0;
      color: var(--text-primary);
    }
    
    .service-tags {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      
      .service-tag {
        margin: 0;
      }
    }
  }
  
  .update-options {
    margin-bottom: 24px;
    
    h4 {
      margin: 0 0 16px 0;
      color: var(--text-primary);
    }
    
    .headers-editor,
    .env-editor {
      margin-top: 12px;
      padding: 12px;
      background-color: var(--bg-light);
      border-radius: 4px;
      
      .header-row,
      .env-row {
        display: flex;
        align-items: center;
        margin-bottom: 8px;
        
        &:last-child {
          margin-bottom: 0;
        }
      }
    }
  }
  
  .update-preview {
    h4 {
      margin: 0 0 12px 0;
      color: var(--text-primary);
    }
    
    .preview-text {
      font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
      font-size: 12px;
    }
  }
}

// 响应式适配
@include respond-to(xs) {
  .batch-update-dialog {
    .header-row,
    .env-row {
      flex-direction: column;
      align-items: stretch;
      gap: 8px;
    }
  }
}
</style>
