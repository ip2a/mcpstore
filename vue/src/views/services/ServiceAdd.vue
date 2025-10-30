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
    
    <!-- 主要内容区域 -->
    <div class="main-content">
      <!-- 服务类型选择卡片 -->
      <el-card class="type-selection-card">
        <template #header>
          <div class="card-header">
            <el-icon><Setting /></el-icon>
            <span>服务类型</span>
          </div>
        </template>

        <el-radio-group v-model="serviceType" @change="handleTypeChange" size="large">
          <el-radio-button label="remote">远程服务</el-radio-button>
          <el-radio-button label="local">本地服务</el-radio-button>
          <el-radio-button label="mcpServers">主流JSON文件格式</el-radio-button>
        </el-radio-group>

        <div class="type-hint">
          <el-icon v-if="serviceType === 'remote'"><Link /></el-icon>
          <el-icon v-else-if="serviceType === 'local'"><FolderOpened /></el-icon>
          <el-icon v-else-if="serviceType === 'mcpServers'"><DocumentCopy /></el-icon>
          <span v-if="serviceType === 'remote'">通过HTTP/SSE连接的远程MCP服务</span>
          <span v-else-if="serviceType === 'local'">本地命令启动的MCP服务</span>
          <span v-else-if="serviceType === 'mcpServers'">标准JSON配置文件格式</span>
        </div>
      </el-card>

      <!-- 服务配置表单卡片 -->
      <el-card class="form-card">
        <template #header>
          <div class="card-header">
            <el-icon><Edit /></el-icon>
            <span>服务配置</span>
          </div>
        </template>
        <div class="section-title">
          <el-icon><Edit /></el-icon>
          <span>服务配置</span>
        </div>
      
        <!-- 远程服务表单 -->
        <el-form
          v-if="serviceType === 'remote'"
          ref="remoteFormRef"
          :model="remoteForm"
          :rules="remoteRules"
          label-position="top"
          class="service-form"
        >
          <div class="form-grid">
            <el-form-item label="服务名称" prop="name">
              <el-input
                v-model="remoteForm.name"
                placeholder="请输入服务名称"
                size="large"
                clearable
              />
            </el-form-item>

            <el-form-item label="传输类型" prop="transport">
              <el-select v-model="remoteForm.transport" placeholder="自动检测" size="large">
                <el-option label="自动检测" value="" />
                <el-option label="HTTP流式" value="streamable-http" />
                <el-option label="SSE" value="sse" />
              </el-select>
            </el-form-item>
          </div>

          <el-form-item label="服务URL" prop="url">
            <el-input
              v-model="remoteForm.url"
              placeholder="https://mcpstore.wiki/mcp"
              size="large"
              clearable
            />
          </el-form-item>

          <!-- 可选配置 -->
          <el-collapse v-model="activeCollapse" class="optional-config">
            <el-collapse-item title="请求头配置" name="headers">
              <div class="config-section">
                <div
                  v-for="(header, index) in remoteForm.headers"
                  :key="index"
                  class="config-item"
                >
                  <el-input
                    v-model="header.key"
                    placeholder="Header名称"
                    size="large"
                  />
                  <el-input
                    v-model="header.value"
                    placeholder="Header值"
                    size="large"
                  />
                  <el-button
                    :icon="Delete"
                    @click="removeHeader(index)"
                    type="danger"
                    text
                    size="large"
                  />
                </div>
                <el-button
                  :icon="Plus"
                  @click="addHeader"
                  type="primary"
                  text
                  size="large"
                >
                  添加请求头
                </el-button>
              </div>
            </el-collapse-item>

            <el-collapse-item title="环境变量" name="env">
              <div class="config-section">
                <div
                  v-for="(env, index) in remoteForm.env"
                  :key="index"
                  class="config-item"
                >
                  <el-input
                    v-model="env.key"
                    placeholder="变量名"
                    size="large"
                  />
                  <el-input
                    v-model="env.value"
                    placeholder="变量值"
                    size="large"
                    :type="env.key.toLowerCase().includes('password') ? 'password' : 'text'"
                  />
                  <el-button
                    :icon="Delete"
                    @click="removeEnv(index)"
                    type="danger"
                    text
                    size="large"
                  />
                </div>
                <el-button
                  :icon="Plus"
                  @click="addEnv"
                  type="primary"
                  text
                  size="large"
                >
                  添加环境变量
                </el-button>
              </div>
            </el-collapse-item>
          </el-collapse>
        </el-form>

        <!-- 本地服务表单 -->
        <el-form
          v-else-if="serviceType === 'local'"
          ref="localFormRef"
          :model="localForm"
          :rules="localRules"
          label-position="top"
          class="service-form"
        >
          <div class="form-grid">
            <el-form-item label="服务名称" prop="name">
              <el-input
                v-model="localForm.name"
                placeholder="请输入服务名称"
                size="large"
                clearable
              />
            </el-form-item>

            <el-form-item label="启动命令" prop="command">
              <el-input
                v-model="localForm.command"
                placeholder="python, node, ./executable"
                size="large"
                clearable
              />
            </el-form-item>
          </div>

          <el-form-item label="工作目录">
            <el-input
              v-model="localForm.working_dir"
              placeholder="留空使用当前目录"
              size="large"
              clearable
            />
          </el-form-item>

          <!-- 可选配置 -->
          <el-collapse v-model="activeCollapse" class="optional-config">
            <el-collapse-item title="命令参数" name="args">
              <div class="config-section">
                <div
                  v-for="(arg, index) in localForm.args"
                  :key="index"
                  class="config-item single"
                >
                  <el-input
                    v-model="localForm.args[index]"
                    placeholder="参数"
                    size="large"
                  />
                  <el-button
                    :icon="Delete"
                    @click="removeArg(index)"
                    type="danger"
                    text
                    size="large"
                  />
                </div>
                <el-button
                  :icon="Plus"
                  @click="addArg"
                  type="primary"
                  text
                  size="large"
                >
                  添加参数
                </el-button>
              </div>
            </el-collapse-item>

            <el-collapse-item title="环境变量" name="env">
              <div class="config-section">
                <div
                  v-for="(env, index) in localForm.env"
                  :key="index"
                  class="config-item"
                >
                  <el-input
                    v-model="env.key"
                    placeholder="变量名"
                    size="large"
                  />
                  <el-input
                    v-model="env.value"
                    placeholder="变量值"
                    size="large"
                    :type="env.key.toLowerCase().includes('password') ? 'password' : 'text'"
                  />
                  <el-button
                    :icon="Delete"
                    @click="removeLocalEnv(index)"
                    type="danger"
                    text
                    size="large"
                  />
                </div>
                <el-button
                  :icon="Plus"
                  @click="addLocalEnv"
                  type="primary"
                  text
                  size="large"
                >
                  添加环境变量
                </el-button>
              </div>
            </el-collapse-item>
          </el-collapse>
        </el-form>
      
        <!-- mcpServers格式表单 -->
        <el-form
          v-else-if="serviceType === 'mcpServers'"
          ref="mcpServersFormRef"
          :model="mcpServersForm"
          label-position="top"
          class="service-form"
        >
          <el-form-item label="配置内容">
            <el-input
              v-model="mcpServersForm.content"
              type="textarea"
              :rows="8"
              placeholder="请输入标准JSON配置文件内容"
              size="large"
            />
          </el-form-item>

          <div class="json-actions">
            <el-button @click="formatJson" size="large">
              <el-icon><Star /></el-icon>
              格式化
            </el-button>
            <el-button @click="validateJson" size="large">
              <el-icon><Check /></el-icon>
              验证
            </el-button>
            <el-button @click="loadExample" size="large">
              <el-icon><Document /></el-icon>
              示例
            </el-button>
          </div>
        </el-form>

        <!-- 提交按钮 -->
        <div class="submit-section">
          <el-button
            type="primary"
            @click="submitForm"
            :loading="submitting"
            size="large"
            class="submit-btn"
          >
            <el-icon><Plus /></el-icon>
            {{ submitting ? '添加中...' : '添加服务' }}
          </el-button>
          <el-button
            @click="resetForm"
            size="large"
          >
            <el-icon><RefreshLeft /></el-icon>
            重置表单
          </el-button>
        </div>
      </el-card>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, reactive } from 'vue'
import { useRouter } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import { ElMessage } from 'element-plus'
import {
  Link, FolderOpened, DocumentCopy, Plus, Delete, Setting, Edit,
  Star, Check, RefreshLeft, Document
} from '@element-plus/icons-vue'

const router = useRouter()
const systemStore = useSystemStore()

// 响应式数据
const serviceType = ref('remote')
const submitting = ref(false)
const activeCollapse = ref([])

// 表单引用
const remoteFormRef = ref()
const localFormRef = ref()
const mcpServersFormRef = ref()

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
        "url": "https://mcpstore.wiki/mcp"
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
    router.push('/services')
  } catch (error) {
    ElMessage.error('服务添加失败: ' + (error.message || error))
  } finally {
    submitting.value = false
  }
}
</script>

<style lang="scss" scoped>
.service-add {
  padding: 20px;
  min-height: 100vh;
  background: var(--el-bg-color-page, #f5f7fa);

  .page-header {
    @include flex-between;
    margin-bottom: 32px;
    max-width: 900px;
    margin-left: auto;
    margin-right: auto;

    .header-left {
      .page-title {
        margin: 0 0 8px 0;
        font-size: 28px;
        font-weight: 600;
      }

      .page-description {
        margin: 0;
        color: var(--el-text-color-secondary);
        font-size: 16px;
      }
    }
  }

  .main-content {
    max-width: 900px;
    margin: 0 auto;
    padding: 0 20px;
  }

  // 卡片样式
  .type-selection-card,
  .form-card {
    margin-bottom: 24px;
    box-shadow: 0 2px 12px 0 rgba(0, 0, 0, 0.08);
    border-radius: 8px;
    overflow: hidden;

    .card-header {
      display: flex;
      align-items: center;
      gap: 8px;
      font-size: 16px;
      font-weight: 600;
      color: var(--el-text-color-primary);
    }
  }

  .type-selection-card {
    .type-hint {
      display: flex;
      align-items: center;
      gap: 8px;
      margin-top: 16px;
      padding: 16px 20px;
      background: var(--el-fill-color-lighter);
      border-radius: 8px;
      color: var(--el-text-color-secondary);
      font-size: 14px;
    }
  }

  .service-form {
    padding: 8px 0;

    .form-grid {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 24px;
      margin-bottom: 24px;
    }

    .optional-config {
      margin-top: 24px;

      .config-section {
        .config-item {
          display: flex;
          align-items: center;
          gap: 12px;
          margin-bottom: 12px;

          &.single {
            .el-input {
              flex: 1;
            }
          }

          .el-input:first-child {
            flex: 1;
          }

          .el-input:nth-child(2) {
            flex: 1;
          }
        }
      }
    }

    // JSON操作按钮样式
    .json-actions {
      margin-top: 16px;
      display: flex;
      gap: 12px;
      justify-content: center;
    }
  }

  .submit-section {
    margin-top: 40px;
    padding: 24px 0;
    border-top: 1px solid var(--el-border-color-lighter);
    display: flex;
    justify-content: center;
    gap: 16px;

    .submit-btn {
      min-width: 180px;
      height: 48px;
      font-size: 16px;
      font-weight: 500;
    }

    .el-button {
      height: 48px;
      font-size: 16px;
    }
  }
}

// 响应式适配
@include respond-to(md) {
  .service-add {
    .service-form .form-grid {
      grid-template-columns: 1fr;
      gap: 16px;
    }
  }
}

@include respond-to(sm) {
  .service-add {
    padding: 16px;

    .page-header {
      flex-direction: column;
      align-items: flex-start;
      gap: 16px;
      margin-bottom: 24px;

      .header-left .page-title {
        font-size: 24px;
      }
    }

    .main-content {
      max-width: none;
      padding: 0;
    }

    .config-item {
      flex-direction: column;
      align-items: stretch;
      gap: 8px;

      .el-input {
        width: 100% !important;
      }
    }

    .submit-section {
      flex-direction: column;
      
      .submit-btn,
      .el-button {
        width: 100%;
        margin: 0;
      }
    }
  }
}
</style>
