<template>
  <div class="service-edit">
    <!-- 页面头部 -->
    <div class="page-header">
      <div class="header-left">
        <h2 class="page-title">
          {{ isEdit ? '编辑服务' : '添加服务' }}
        </h2>
        <p class="page-description">
          {{ isEdit ? `编辑服务 "${serviceName}"` : '配置新的MCP服务' }}
        </p>
      </div>
      <div class="header-right">
        <el-button @click="$router.back()">
          返回
        </el-button>
      </div>
    </div>

    <!-- 编辑表单 -->
    <el-card class="form-card">
      <el-form
        ref="formRef"
        :model="form"
        :rules="rules"
        label-width="120px"
        @submit.prevent="handleSubmit"
      >
        <!-- 基本信息 -->
        <div class="form-section">
          <h3 class="section-title">基本信息</h3>
          
          <el-form-item label="服务名称" prop="name">
            <el-input
              v-model="form.name"
              placeholder="请输入服务名称"
              :disabled="isEdit"
            />
            <div class="form-tip">
              {{ isEdit ? '服务名称不可修改' : '服务名称必须唯一' }}
            </div>
          </el-form-item>

          <el-form-item label="服务类型" prop="serviceType">
            <el-radio-group v-model="form.serviceType" @change="handleServiceTypeChange">
              <el-radio value="remote">远程服务</el-radio>
              <el-radio value="local">本地服务</el-radio>
            </el-radio-group>
          </el-form-item>
        </div>

        <!-- 远程服务配置 -->
        <div v-if="form.serviceType === 'remote'" class="form-section">
          <h3 class="section-title">远程服务配置</h3>
          
          <el-form-item label="服务URL" prop="url">
            <el-input
              v-model="form.url"
              placeholder="https://api.example.com/mcp"
            />
          </el-form-item>

          <el-form-item label="传输类型" prop="transport">
            <el-select v-model="form.transport" placeholder="选择传输类型">
              <el-option label="HTTP" value="http" />
              <el-option label="Streamable HTTP" value="streamable-http" />
              <el-option label="SSE" value="sse" />
            </el-select>
          </el-form-item>

          <!-- HTTP Headers -->
          <el-form-item label="请求头">
            <div class="key-value-editor">
              <div
                v-for="(header, index) in form.headers"
                :key="index"
                class="key-value-row"
              >
                <el-input
                  v-model="header.key"
                  placeholder="Header名称"
                  class="key-input"
                />
                <el-input
                  v-model="header.value"
                  placeholder="Header值"
                  class="value-input"
                />
                <el-button
                  type="danger"
                  :icon="Delete"
                  @click="removeHeader(index)"
                />
              </div>
              <el-button
                type="primary"
                :icon="Plus"
                @click="addHeader"
                class="add-button"
              >
                添加Header
              </el-button>
            </div>
          </el-form-item>
        </div>

        <!-- 本地服务配置 -->
        <div v-if="form.serviceType === 'local'" class="form-section">
          <h3 class="section-title">本地服务配置</h3>
          
          <el-form-item label="执行命令" prop="command">
            <el-input
              v-model="form.command"
              placeholder="python, node, ./my-script"
            />
          </el-form-item>

          <el-form-item label="命令参数">
            <div class="array-editor">
              <div
                v-for="(arg, index) in form.args"
                :key="index"
                class="array-row"
              >
                <el-input
                  v-model="form.args[index]"
                  placeholder="参数"
                  class="array-input"
                />
                <el-button
                  type="danger"
                  :icon="Delete"
                  @click="removeArg(index)"
                />
              </div>
              <el-button
                type="primary"
                :icon="Plus"
                @click="addArg"
                class="add-button"
              >
                添加参数
              </el-button>
            </div>
          </el-form-item>

          <el-form-item label="工作目录">
            <el-input
              v-model="form.working_dir"
              placeholder="留空使用默认目录"
            />
          </el-form-item>
        </div>

        <!-- 环境变量 -->
        <div class="form-section">
          <h3 class="section-title">环境变量</h3>
          
          <div class="key-value-editor">
            <div
              v-for="(env, index) in form.env"
              :key="index"
              class="key-value-row"
            >
              <el-input
                v-model="env.key"
                placeholder="变量名"
                class="key-input"
              />
              <el-input
                v-model="env.value"
                placeholder="变量值"
                class="value-input"
                :type="env.key.toLowerCase().includes('password') || env.key.toLowerCase().includes('key') ? 'password' : 'text'"
                show-password
              />
              <el-button
                type="danger"
                :icon="Delete"
                @click="removeEnv(index)"
              />
            </div>
            <el-button
              type="primary"
              :icon="Plus"
              @click="addEnv"
              class="add-button"
            >
              添加环境变量
            </el-button>
          </div>
        </div>

        <!-- 高级配置 -->
        <div class="form-section">
          <h3 class="section-title">高级配置</h3>
          
          <el-form-item label="超时时间">
            <el-input-number
              v-model="form.timeout"
              :min="1"
              :max="300"
              placeholder="秒"
            />
            <span class="form-tip">连接超时时间（秒）</span>
          </el-form-item>

          <el-form-item label="保持连接">
            <el-switch v-model="form.keep_alive" />
            <span class="form-tip">是否保持长连接</span>
          </el-form-item>
        </div>

        <!-- 操作按钮 -->
        <div class="form-actions">
          <el-button @click="$router.back()">
            取消
          </el-button>
          <el-button
            type="primary"
            @click="handleSubmit"
            :loading="submitting"
          >
            {{ isEdit ? '更新服务' : '添加服务' }}
          </el-button>
          <el-button
            v-if="isEdit"
            type="warning"
            @click="handlePatchUpdate"
            :loading="patching"
          >
            增量更新
          </el-button>
        </div>
      </el-form>
    </el-card>

    <!-- 配置预览 -->
    <el-card class="preview-card">
      <template #header>
        <div class="card-header">
          <span>配置预览</span>
          <el-button
            type="text"
            :icon="CopyDocument"
            @click="copyConfig"
          >
            复制配置
          </el-button>
        </div>
      </template>
      
      <el-input
        v-model="configPreview"
        type="textarea"
        :rows="12"
        readonly
        class="config-preview"
      />
    </el-card>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import { ElMessage, ElMessageBox } from 'element-plus'
import { api } from '@/api'
import {
  Plus, Delete, CopyDocument
} from '@element-plus/icons-vue'

const route = useRoute()
const router = useRouter()
const systemStore = useSystemStore()

// 响应式数据
const formRef = ref()
const submitting = ref(false)
const patching = ref(false)

// 判断是否为编辑模式
const isEdit = computed(() => !!route.params.serviceName)
const serviceName = computed(() => route.params.serviceName)
const agentId = computed(() => route.query.agent)

// 表单数据
const form = ref({
  name: '',
  serviceType: 'remote',
  url: '',
  transport: 'http',
  command: '',
  args: [],
  working_dir: '',
  headers: [],
  env: [],
  timeout: 30,
  keep_alive: true
})

// 表单验证规则
const rules = {
  name: [
    { required: true, message: '请输入服务名称', trigger: 'blur' },
    { pattern: /^[a-zA-Z0-9_-]+$/, message: '服务名称只能包含字母、数字、下划线和连字符', trigger: 'blur' }
  ],
  serviceType: [
    { required: true, message: '请选择服务类型', trigger: 'change' }
  ],
  url: [
    { 
      required: true, 
      message: '请输入服务URL', 
      trigger: 'blur',
      validator: (rule, value, callback) => {
        if (form.value.serviceType === 'remote' && !value) {
          callback(new Error('远程服务必须提供URL'))
        } else if (value && !value.startsWith('http')) {
          callback(new Error('URL必须以http或https开头'))
        } else {
          callback()
        }
      }
    }
  ],
  command: [
    { 
      required: true, 
      message: '请输入执行命令', 
      trigger: 'blur',
      validator: (rule, value, callback) => {
        if (form.value.serviceType === 'local' && !value) {
          callback(new Error('本地服务必须提供执行命令'))
        } else {
          callback()
        }
      }
    }
  ]
}

// 配置预览
const configPreview = computed(() => {
  const config = buildServiceConfig()
  return JSON.stringify(config, null, 2)
})

// 方法
const buildServiceConfig = () => {
  const config = {
    name: form.value.name
  }

  if (form.value.serviceType === 'remote') {
    config.url = form.value.url
    if (form.value.transport) {
      config.transport = form.value.transport
    }
    
    // 处理headers
    const headers = {}
    form.value.headers.forEach(header => {
      if (header.key && header.value) {
        headers[header.key] = header.value
      }
    })
    if (Object.keys(headers).length > 0) {
      config.headers = headers
    }
  } else {
    config.command = form.value.command
    if (form.value.args.length > 0) {
      config.args = form.value.args.filter(arg => arg.trim())
    }
    if (form.value.working_dir) {
      config.working_dir = form.value.working_dir
    }
  }

  // 处理环境变量
  const env = {}
  form.value.env.forEach(envVar => {
    if (envVar.key && envVar.value) {
      env[envVar.key] = envVar.value
    }
  })
  if (Object.keys(env).length > 0) {
    config.env = env
  }

  // 高级配置
  if (form.value.timeout !== 30) {
    config.timeout = form.value.timeout
  }
  if (!form.value.keep_alive) {
    config.keep_alive = form.value.keep_alive
  }

  return config
}

const loadServiceConfig = async () => {
  if (!isEdit.value) return

  try {
    console.log('Loading service config for:', serviceName.value)
    const apiModule = agentId.value ? api.agent : api.store
    const response = agentId.value
      ? await apiModule.getServiceInfo(agentId.value, serviceName.value)
      : await apiModule.getServiceInfo(serviceName.value)

    console.log('API response:', response)

    if (response.success) {
      // 由于响应拦截器已经返回了data，所以response就是API响应的数据
      // 正确的访问路径是response.data.service
      const serviceInfo = response.data.service
      console.log('Service info:', serviceInfo)

      // 填充表单
      form.value.name = serviceInfo.name
      form.value.serviceType = serviceInfo.url ? 'remote' : 'local'
      
      if (serviceInfo.url) {
        form.value.url = serviceInfo.url
        // 映射transport_type到transport，并处理下划线转连字符
        const transportType = serviceInfo.transport_type || 'http'
        form.value.transport = transportType.replace(/_/g, '-')

        // 处理headers - 从API响应中可能没有headers字段，需要安全处理
        if (serviceInfo.headers) {
          form.value.headers = Object.entries(serviceInfo.headers).map(([key, value]) => ({
            key, value
          }))
        }
      }

      if (serviceInfo.command) {
        form.value.command = serviceInfo.command
        form.value.args = serviceInfo.args || []
        form.value.working_dir = serviceInfo.working_dir || ''
      }

      // 处理环境变量
      if (serviceInfo.env) {
        form.value.env = Object.entries(serviceInfo.env).map(([key, value]) => ({
          key, value
        }))
      }

      form.value.timeout = serviceInfo.timeout || 30
      form.value.keep_alive = serviceInfo.keep_alive !== false

      console.log('Form populated successfully:', form.value)
    } else {
      console.error('API response not successful:', response)
      ElMessage.error(response.message || '获取服务信息失败')
      router.back()
    }
  } catch (error) {
    console.error('Load service config error:', error)
    ElMessage.error(`加载服务配置失败: ${error.message || error}`)
    router.back()
  }
}

const handleServiceTypeChange = () => {
  // 清空相关字段
  if (form.value.serviceType === 'remote') {
    form.value.command = ''
    form.value.args = []
    form.value.working_dir = ''
  } else {
    form.value.url = ''
    form.value.transport = 'http'
    form.value.headers = []
  }
}

const addHeader = () => {
  form.value.headers.push({ key: '', value: '' })
}

const removeHeader = (index) => {
  form.value.headers.splice(index, 1)
}

const addArg = () => {
  form.value.args.push('')
}

const removeArg = (index) => {
  form.value.args.splice(index, 1)
}

const addEnv = () => {
  form.value.env.push({ key: '', value: '' })
}

const removeEnv = (index) => {
  form.value.env.splice(index, 1)
}

const handleSubmit = async () => {
  try {
    await formRef.value.validate()
    
    submitting.value = true
    const config = buildServiceConfig()
    
    if (isEdit.value) {
      // 更新服务（完全替换）
      const apiModule = agentId.value ? api.agent : api.store
      const response = agentId.value
        ? await apiModule.updateService(agentId.value, serviceName.value, config)
        : await apiModule.updateService(serviceName.value, config)
      
      if (response.data.success) {
        ElMessage.success('服务更新成功')
        router.back()
      } else {
        ElMessage.error(response.data.message || '服务更新失败')
      }
    } else {
      // 添加新服务
      const apiModule = agentId.value ? api.agent : api.store
      const response = agentId.value
        ? await apiModule.addService(agentId.value, config)
        : await apiModule.addService(config)
      
      if (response.data.success) {
        ElMessage.success('服务添加成功')
        router.back()
      } else {
        ElMessage.error(response.data.message || '服务添加失败')
      }
    }
  } catch (error) {
    console.error('表单验证失败:', error)
  } finally {
    submitting.value = false
  }
}

const handlePatchUpdate = async () => {
  if (!isEdit.value) return
  
  try {
    await formRef.value.validate()
    
    patching.value = true
    const updates = buildServiceConfig()
    delete updates.name // 增量更新不包含名称
    
    const apiModule = agentId.value ? api.agent : api.store
    const response = agentId.value
      ? await apiModule.patchService(agentId.value, serviceName.value, updates)
      : await apiModule.patchService(serviceName.value, updates)
    
    if (response.data.success) {
      ElMessage.success('服务增量更新成功')
      router.back()
    } else {
      ElMessage.error(response.data.message || '服务增量更新失败')
    }
  } catch (error) {
    console.error('表单验证失败:', error)
  } finally {
    patching.value = false
  }
}

const copyConfig = async () => {
  try {
    await navigator.clipboard.writeText(configPreview.value)
    ElMessage.success('配置已复制到剪贴板')
  } catch (error) {
    ElMessage.error('复制失败')
  }
}

// 生命周期
onMounted(async () => {
  if (isEdit.value) {
    await loadServiceConfig()
  }
})
</script>

<style lang="scss" scoped>
.service-edit {
  .page-header {
    @include flex-between;
    margin-bottom: 20px;
    
    .header-left {
      .page-title {
        margin: 0 0 4px 0;
        font-size: 24px;
        font-weight: var(--font-weight-medium);
      }
      
      .page-description {
        margin: 0;
        color: var(--text-secondary);
      }
    }
  }
  
  .form-card {
    margin-bottom: 20px;
    
    .form-section {
      margin-bottom: 32px;
      
      .section-title {
        margin: 0 0 16px 0;
        font-size: 16px;
        font-weight: var(--font-weight-medium);
        color: var(--text-primary);
        border-bottom: 1px solid var(--border-light);
        padding-bottom: 8px;
      }
    }
    
    .form-tip {
      font-size: var(--font-size-xs);
      color: var(--text-secondary);
      margin-top: 4px;
    }
    
    .key-value-editor,
    .array-editor {
      .key-value-row,
      .array-row {
        display: flex;
        gap: 8px;
        margin-bottom: 8px;
        align-items: center;
        
        .key-input,
        .array-input {
          flex: 1;
        }
        
        .value-input {
          flex: 2;
        }
      }
      
      .add-button {
        margin-top: 8px;
      }
    }
    
    .form-actions {
      display: flex;
      gap: 12px;
      justify-content: flex-end;
      margin-top: 32px;
      padding-top: 20px;
      border-top: 1px solid var(--border-light);
    }
  }
  
  .preview-card {
    .card-header {
      @include flex-between;
      align-items: center;
    }
    
    .config-preview {
      font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
      font-size: 12px;
    }
  }
}

// 响应式适配
@include respond-to(xs) {
  .service-edit {
    .page-header {
      flex-direction: column;
      align-items: flex-start;
      gap: 16px;
    }
    
    .key-value-row,
    .array-row {
      flex-direction: column;
      align-items: stretch;
    }
    
    .form-actions {
      flex-direction: column;
      
      .el-button {
        width: 100%;
      }
    }
  }
}
</style>
