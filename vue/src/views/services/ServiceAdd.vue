<template>
  <div class="service-add">
    <!-- 页面头部 -->
    <div class="page-header">
      <div class="header-left">
        <h2 class="page-title">添加服务</h2>
        <p class="page-description">注册新的MCP服务到系统中</p>
      </div>
      <div class="header-right">
        <el-button @click="$router.back()">
          返回
        </el-button>
      </div>
    </div>
    
    <!-- 服务类型选择 -->
    <el-card class="type-selection-card">
      <template #header>
        <span>选择服务类型</span>
      </template>
      
      <el-radio-group v-model="serviceType" @change="handleTypeChange">
        <el-radio-button label="remote">远程服务</el-radio-button>
        <el-radio-button label="local">本地服务</el-radio-button>
        <el-radio-button label="mcpServers">mcpServers格式</el-radio-button>
      </el-radio-group>
      
      <div class="type-description">
        <div v-if="serviceType === 'remote'" class="description-item">
          <el-icon><Link /></el-icon>
          <span>通过HTTP/SSE连接的远程MCP服务</span>
        </div>
        <div v-else-if="serviceType === 'local'" class="description-item">
          <el-icon><FolderOpened /></el-icon>
          <span>本地命令启动的MCP服务，支持进程管理</span>
        </div>
        <div v-else-if="serviceType === 'mcpServers'" class="description-item">
          <el-icon><DocumentCopy /></el-icon>
          <span>标准mcpServers配置格式</span>
        </div>
      </div>
    </el-card>
    
    <!-- 服务配置表单 -->
    <el-card class="form-card">
      <template #header>
        <span>服务配置</span>
      </template>
      
      <!-- 远程服务表单 -->
      <el-form
        v-if="serviceType === 'remote'"
        ref="remoteFormRef"
        :model="remoteForm"
        :rules="remoteRules"
        label-width="120px"
      >
        <el-form-item label="服务名称" prop="name">
          <el-input 
            v-model="remoteForm.name" 
            placeholder="请输入服务名称"
            clearable
          />
        </el-form-item>
        
        <el-form-item label="服务URL" prop="url">
          <el-input
            v-model="remoteForm.url"
            placeholder="http://mcpstore.wiki/mcp"
            clearable
          />
        </el-form-item>
        
        <el-form-item label="传输类型" prop="transport">
          <el-select v-model="remoteForm.transport" placeholder="自动检测">
            <el-option label="自动检测" value="" />
            <el-option label="HTTP流式" value="streamable-http" />
            <el-option label="SSE" value="sse" />
          </el-select>
        </el-form-item>
        
        <el-form-item label="请求头">
          <div class="headers-input">
            <div 
              v-for="(header, index) in remoteForm.headers" 
              :key="index"
              class="header-item"
            >
              <el-input 
                v-model="header.key" 
                placeholder="Header名称"
                style="width: 40%"
              />
              <el-input 
                v-model="header.value" 
                placeholder="Header值"
                style="width: 40%"
              />
              <el-button 
                :icon="Delete" 
                @click="removeHeader(index)"
                type="danger"
                text
              />
            </div>
            <el-button 
              :icon="Plus" 
              @click="addHeader"
              type="primary"
              text
            >
              添加请求头
            </el-button>
          </div>
        </el-form-item>
        
        <el-form-item label="环境变量">
          <div class="env-input">
            <div 
              v-for="(env, index) in remoteForm.env" 
              :key="index"
              class="env-item"
            >
              <el-input 
                v-model="env.key" 
                placeholder="变量名"
                style="width: 40%"
              />
              <el-input 
                v-model="env.value" 
                placeholder="变量值"
                style="width: 40%"
                :type="env.key.toLowerCase().includes('password') ? 'password' : 'text'"
              />
              <el-button 
                :icon="Delete" 
                @click="removeEnv(index)"
                type="danger"
                text
              />
            </div>
            <el-button 
              :icon="Plus" 
              @click="addEnv"
              type="primary"
              text
            >
              添加环境变量
            </el-button>
          </div>
        </el-form-item>
      </el-form>
      
      <!-- 本地服务表单 -->
      <el-form
        v-else-if="serviceType === 'local'"
        ref="localFormRef"
        :model="localForm"
        :rules="localRules"
        label-width="120px"
      >
        <el-form-item label="服务名称" prop="name">
          <el-input 
            v-model="localForm.name" 
            placeholder="请输入服务名称"
            clearable
          />
        </el-form-item>
        
        <el-form-item label="启动命令" prop="command">
          <el-input 
            v-model="localForm.command" 
            placeholder="python, node, ./executable"
            clearable
          />
        </el-form-item>
        
        <el-form-item label="命令参数">
          <div class="args-input">
            <div 
              v-for="(arg, index) in localForm.args" 
              :key="index"
              class="arg-item"
            >
              <el-input 
                v-model="localForm.args[index]" 
                placeholder="参数"
                style="width: 80%"
              />
              <el-button 
                :icon="Delete" 
                @click="removeArg(index)"
                type="danger"
                text
              />
            </div>
            <el-button 
              :icon="Plus" 
              @click="addArg"
              type="primary"
              text
            >
              添加参数
            </el-button>
          </div>
        </el-form-item>
        
        <el-form-item label="工作目录">
          <el-input 
            v-model="localForm.working_dir" 
            placeholder="留空使用当前目录"
            clearable
          />
        </el-form-item>
        
        <el-form-item label="环境变量">
          <div class="env-input">
            <div 
              v-for="(env, index) in localForm.env" 
              :key="index"
              class="env-item"
            >
              <el-input 
                v-model="env.key" 
                placeholder="变量名"
                style="width: 40%"
              />
              <el-input 
                v-model="env.value" 
                placeholder="变量值"
                style="width: 40%"
                :type="env.key.toLowerCase().includes('password') ? 'password' : 'text'"
              />
              <el-button 
                :icon="Delete" 
                @click="removeLocalEnv(index)"
                type="danger"
                text
              />
            </div>
            <el-button 
              :icon="Plus" 
              @click="addLocalEnv"
              type="primary"
              text
            >
              添加环境变量
            </el-button>
          </div>
        </el-form-item>
      </el-form>
      
      <!-- mcpServers格式表单 -->
      <div v-else-if="serviceType === 'mcpServers'" class="mcpservers-form">
        <el-form-item label="配置内容">
          <el-input
            v-model="mcpServersForm.content"
            type="textarea"
            :rows="15"
            placeholder="请输入mcpServers格式的JSON配置"
          />
        </el-form-item>
        
        <div class="json-actions">
          <el-button @click="formatJson">格式化JSON</el-button>
          <el-button @click="validateJson">验证JSON</el-button>
          <el-button @click="loadExample">加载示例</el-button>
        </div>
      </div>
    </el-card>
    
    <!-- 预览和提交 -->
    <el-card class="preview-card">
      <template #header>
        <span>配置预览</span>
      </template>
      
      <el-alert
        v-if="configPreview"
        title="配置预览"
        type="info"
        :closable="false"
        style="margin-bottom: 16px"
      >
        <pre>{{ configPreview }}</pre>
      </el-alert>
      
      <div class="submit-actions">
        <el-button 
          type="primary" 
          @click="submitForm"
          :loading="submitting"
          size="large"
        >
          添加服务
        </el-button>
        <el-button 
          @click="resetForm"
          size="large"
        >
          重置表单
        </el-button>
      </div>
    </el-card>
  </div>
</template>

<script setup>
import { ref, computed, reactive } from 'vue'
import { useRouter } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import { ElMessage } from 'element-plus'
import {
  Link, FolderOpened, DocumentCopy, Plus, Delete
} from '@element-plus/icons-vue'

const router = useRouter()
const systemStore = useSystemStore()

// 响应式数据
const serviceType = ref('remote')
const submitting = ref(false)

// 表单引用
const remoteFormRef = ref()
const localFormRef = ref()

// 远程服务表单
const remoteForm = reactive({
  name: '',
  url: '',
  transport: '',
  headers: [],
  env: []
})

// 本地服务表单
const localForm = reactive({
  name: '',
  command: '',
  args: [],
  working_dir: '',
  env: []
})

// mcpServers表单
const mcpServersForm = reactive({
  content: ''
})

// 表单验证规则
const remoteRules = {
  name: [
    { required: true, message: '请输入服务名称', trigger: 'blur' }
  ],
  url: [
    { required: true, message: '请输入服务URL', trigger: 'blur' },
    { type: 'url', message: '请输入有效的URL', trigger: 'blur' }
  ]
}

const localRules = {
  name: [
    { required: true, message: '请输入服务名称', trigger: 'blur' }
  ],
  command: [
    { required: true, message: '请输入启动命令', trigger: 'blur' }
  ]
}

// 计算属性
const configPreview = computed(() => {
  try {
    let config = {}
    
    if (serviceType.value === 'remote') {
      config = {
        name: remoteForm.name,
        url: remoteForm.url,
        transport: remoteForm.transport || undefined,
        headers: remoteForm.headers.reduce((acc, h) => {
          if (h.key && h.value) acc[h.key] = h.value
          return acc
        }, {}),
        env: remoteForm.env.reduce((acc, e) => {
          if (e.key && e.value) acc[e.key] = e.value
          return acc
        }, {})
      }
    } else if (serviceType.value === 'local') {
      config = {
        name: localForm.name,
        command: localForm.command,
        args: localForm.args.filter(arg => arg.trim()),
        working_dir: localForm.working_dir || undefined,
        env: localForm.env.reduce((acc, e) => {
          if (e.key && e.value) acc[e.key] = e.value
          return acc
        }, {})
      }
    } else if (serviceType.value === 'mcpServers') {
      try {
        config = JSON.parse(mcpServersForm.content || '{}')
      } catch {
        return 'JSON格式错误'
      }
    }
    
    return JSON.stringify(config, null, 2)
  } catch (error) {
    return '配置生成错误'
  }
})

// 方法
const handleTypeChange = () => {
  resetForm()
}

const addHeader = () => {
  remoteForm.headers.push({ key: '', value: '' })
}

const removeHeader = (index) => {
  remoteForm.headers.splice(index, 1)
}

const addEnv = () => {
  remoteForm.env.push({ key: '', value: '' })
}

const removeEnv = (index) => {
  remoteForm.env.splice(index, 1)
}

const addArg = () => {
  localForm.args.push('')
}

const removeArg = (index) => {
  localForm.args.splice(index, 1)
}

const addLocalEnv = () => {
  localForm.env.push({ key: '', value: '' })
}

const removeLocalEnv = (index) => {
  localForm.env.splice(index, 1)
}

const formatJson = () => {
  try {
    const parsed = JSON.parse(mcpServersForm.content)
    mcpServersForm.content = JSON.stringify(parsed, null, 2)
    ElMessage.success('JSON格式化成功')
  } catch (error) {
    ElMessage.error('JSON格式错误')
  }
}

const validateJson = () => {
  try {
    JSON.parse(mcpServersForm.content)
    ElMessage.success('JSON格式正确')
  } catch (error) {
    ElMessage.error('JSON格式错误: ' + error.message)
  }
}

const loadExample = () => {
  const example = {
    mcpServers: {
      "mcpstore-wiki": {
        "url": "http://mcpstore.wiki/mcp"
      },
      "howtocook": {
        "command": "npx",
        "args": ["-y", "howtocook-mcp"]
      }
    }
  }
  mcpServersForm.content = JSON.stringify(example, null, 2)
}

const resetForm = () => {
  if (serviceType.value === 'remote') {
    Object.assign(remoteForm, {
      name: '',
      url: '',
      transport: '',
      headers: [],
      env: []
    })
    remoteFormRef.value?.resetFields()
  } else if (serviceType.value === 'local') {
    Object.assign(localForm, {
      name: '',
      command: '',
      args: [],
      working_dir: '',
      env: []
    })
    localFormRef.value?.resetFields()
  } else if (serviceType.value === 'mcpServers') {
    mcpServersForm.content = ''
  }
}

const submitForm = async () => {
  try {
    submitting.value = true
    
    let config = {}
    
    // 验证表单
    if (serviceType.value === 'remote') {
      await remoteFormRef.value.validate()
      config = {
        name: remoteForm.name,
        url: remoteForm.url
      }
      
      if (remoteForm.transport) {
        config.transport = remoteForm.transport
      }
      
      if (remoteForm.headers.length > 0) {
        config.headers = remoteForm.headers.reduce((acc, h) => {
          if (h.key && h.value) acc[h.key] = h.value
          return acc
        }, {})
      }
      
      if (remoteForm.env.length > 0) {
        config.env = remoteForm.env.reduce((acc, e) => {
          if (e.key && e.value) acc[e.key] = e.value
          return acc
        }, {})
      }
    } else if (serviceType.value === 'local') {
      await localFormRef.value.validate()
      config = {
        name: localForm.name,
        command: localForm.command
      }
      
      if (localForm.args.length > 0) {
        config.args = localForm.args.filter(arg => arg.trim())
      }
      
      if (localForm.working_dir) {
        config.working_dir = localForm.working_dir
      }
      
      if (localForm.env.length > 0) {
        config.env = localForm.env.reduce((acc, e) => {
          if (e.key && e.value) acc[e.key] = e.value
          return acc
        }, {})
      }
    } else if (serviceType.value === 'mcpServers') {
      config = JSON.parse(mcpServersForm.content)
    }
    
    // 提交配置
    await systemStore.addService(config)
    
    ElMessage.success('服务添加成功')
    router.push('/services/list')
  } catch (error) {
    ElMessage.error('服务添加失败: ' + (error.message || error))
  } finally {
    submitting.value = false
  }
}
</script>

<style lang="scss" scoped>
.service-add {
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
  
  .type-selection-card,
  .form-card,
  .preview-card {
    margin-bottom: 20px;
  }
  
  .type-description {
    margin-top: 16px;
    
    .description-item {
      display: flex;
      align-items: center;
      gap: 8px;
      color: var(--text-secondary);
      font-size: var(--font-size-sm);
    }
  }
  
  .headers-input,
  .env-input,
  .args-input {
    .header-item,
    .env-item,
    .arg-item {
      display: flex;
      align-items: center;
      gap: 8px;
      margin-bottom: 8px;
    }
  }
  
  .mcpservers-form {
    .json-actions {
      margin-top: 12px;
      display: flex;
      gap: 8px;
    }
  }
  
  .preview-card {
    pre {
      background: var(--bg-color-page);
      padding: 12px;
      border-radius: var(--border-radius-base);
      font-size: var(--font-size-sm);
      max-height: 300px;
      overflow-y: auto;
    }
    
    .submit-actions {
      display: flex;
      gap: 12px;
      justify-content: center;
      margin-top: 20px;
    }
  }
}

// 响应式适配
@include respond-to(xs) {
  .service-add {
    .page-header {
      flex-direction: column;
      align-items: flex-start;
      gap: 16px;
    }
    
    .header-item,
    .env-item {
      flex-direction: column;
      align-items: stretch;
      
      .el-input {
        width: 100% !important;
        margin-bottom: 8px;
      }
    }
    
    .submit-actions {
      flex-direction: column;
      
      .el-button {
        width: 100%;
      }
    }
  }
}
</style>
