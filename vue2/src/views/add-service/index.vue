<template>
  <div class="add-service-page art-full-height">
    <el-card class="art-table-card" shadow="never">
      <div class="page-header">
        <h2 class="page-title">添加服务</h2>
        <p class="page-subtitle">选择服务类型并填写相关配置信息</p>
      </div>

      <!-- 服务类型选择 -->
      <div class="service-type-selector">
        <el-radio-group v-model="serviceType" size="large" @change="handleTypeChange">
          <el-radio-button value="remote">远程服务</el-radio-button>
          <el-radio-button value="local">本地服务</el-radio-button>
          <el-radio-button value="json">MCP JSON 配置</el-radio-button>
        </el-radio-group>
      </div>

      <!-- 表单区域 -->
      <div class="form-container">
        <el-form
          ref="formRef"
          :model="formData"
          :rules="formRules"
          label-width="120px"
          size="large"
        >
          <!-- 远程服务表单 -->
          <template v-if="serviceType === 'remote'">
            <div class="form-section">
              <h3 class="section-title">远程服务配置</h3>
              <el-form-item label="服务名称" prop="name">
                <el-input
                  v-model="formData.name"
                  placeholder="请输入服务名称，如：mcpstore-wiki"
                  clearable
                />
              </el-form-item>
              <el-form-item label="服务地址" prop="url">
                <el-input
                  v-model="formData.url"
                  placeholder="请输入服务地址，如：http://mcpstore.wiki/mcp"
                  clearable
                />
              </el-form-item>
              <el-form-item label="描述信息" prop="description">
                <el-input
                  v-model="formData.description"
                  type="textarea"
                  :rows="3"
                  placeholder="请输入服务描述信息（可选）"
                />
              </el-form-item>
            </div>
          </template>

          <!-- 本地服务表单 -->
          <template v-if="serviceType === 'local'">
            <div class="form-section">
              <h3 class="section-title">本地服务配置</h3>
              <el-form-item label="服务名称" prop="name">
                <el-input
                  v-model="formData.name"
                  placeholder="请输入服务名称，如：local-service"
                  clearable
                />
              </el-form-item>
              <el-form-item label="启动命令" prop="command">
                <el-input
                  v-model="formData.command"
                  placeholder="请输入启动命令，如：npx -y howtocook-mcp"
                  clearable
                />
              </el-form-item>
              <el-form-item label="描述信息" prop="description">
                <el-input
                  v-model="formData.description"
                  type="textarea"
                  :rows="3"
                  placeholder="请输入服务描述信息（可选）"
                />
              </el-form-item>
            </div>
          </template>

          <!-- MCP JSON 配置 -->
          <template v-if="serviceType === 'json'">
            <div class="form-section">
              <h3 class="section-title">MCP JSON 配置</h3>
              <el-form-item label="配置内容" prop="jsonConfig">
                <el-input
                  v-model="formData.jsonConfig"
                  type="textarea"
                  :rows="15"
                  placeholder="请输入 MCP JSON 配置内容"
                  class="json-editor"
                />
              </el-form-item>
              <div class="json-example">
                <el-collapse>
                  <el-collapse-item title="查看配置示例" name="example">
                    <pre class="json-code">{{jsonExample}}</pre>
                  </el-collapse-item>
                </el-collapse>
              </div>
            </div>
          </template>

          <!-- 操作按钮 -->
          <div class="form-actions">
            <el-button size="large" @click="handleReset">重置</el-button>
            <el-button size="large" @click="handlePreview">预览配置</el-button>
            <el-button type="primary" size="large" :loading="submitLoading" @click="handleSubmit">
              添加服务
            </el-button>
          </div>
        </el-form>
      </div>
    </el-card>

    <!-- 预览对话框 -->
    <el-dialog v-model="previewVisible" title="配置预览" width="60%" align-center>
      <pre class="preview-content">{{ previewContent }}</pre>
      <template #footer>
        <el-button @click="previewVisible = false">关闭</el-button>
        <el-button type="primary" @click="confirmAdd">确认添加</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { reactive, ref, computed } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import { useRouter } from 'vue-router'
import { dashboardApi } from '../../mcp/api/dashboard'

defineOptions({ name: 'AddService' })

const router = useRouter()

// 表单引用
const formRef = ref<FormInstance>()

// 服务类型
const serviceType = ref<'remote' | 'local' | 'json'>('remote')

// 提交状态
const submitLoading = ref(false)

// 预览对话框
const previewVisible = ref(false)

// 表单数据
const formData = reactive({
  name: '',
  url: '',
  command: '',
  description: '',
  jsonConfig: ''
})

// JSON 编辑器工具栏配置
const jsonEditorToolbar = [
  'undo', 'redo', '|',
  'bold', 'italic', '|',
  'code', 'codeBlock'
]

// JSON 配置示例
const jsonExample = `{
  "mcpServers": {
    "example-service": {
      "command": "npx",
      "args": ["-y", "example-mcp"],
      "env": {
        "API_KEY": "your-api-key"
      }
    },
    "remote-service": {
      "url": "http://example.com/mcp",
      "headers": {
        "Authorization": "Bearer token"
      }
    }
  }
}`

// 表单验证规则
const formRules = computed<FormRules>(() => {
  const baseRules = {
    name: [
      { required: true, message: '请输入服务名称', trigger: 'blur' },
      { min: 2, max: 50, message: '长度在 2 到 50 个字符', trigger: 'blur' }
    ],
    description: [
      { max: 200, message: '描述信息不能超过 200 个字符', trigger: 'blur' }
    ]
  }

  if (serviceType.value === 'remote') {
    return {
      ...baseRules,
      url: [
        { required: true, message: '请输入服务地址', trigger: 'blur' },
        { type: 'url', message: '请输入正确的 URL 格式', trigger: 'blur' }
      ]
    }
  } else if (serviceType.value === 'local') {
    return {
      ...baseRules,
      command: [
        { required: true, message: '请输入启动命令', trigger: 'blur' },
        { min: 3, max: 200, message: '长度在 3 到 200 个字符', trigger: 'blur' }
      ]
    }
  } else {
    return {
      jsonConfig: [
        { required: true, message: '请输入 JSON 配置内容', trigger: 'blur' }
      ]
    }
  }
})

// 预览内容
const previewContent = computed(() => {
  if (serviceType.value === 'json') {
    try {
      return JSON.stringify(JSON.parse(formData.jsonConfig), null, 2)
    } catch {
      return formData.jsonConfig
    }
  } else {
    const config: any = {
      name: formData.name,
      type: serviceType.value,
      description: formData.description
    }
    
    if (serviceType.value === 'remote') {
      config.url = formData.url
    } else if (serviceType.value === 'local') {
      config.command = formData.command
    }
    
    return JSON.stringify(config, null, 2)
  }
})

// 处理类型切换
const handleTypeChange = () => {
  // 清空表单数据
  Object.keys(formData).forEach(key => {
    formData[key as keyof typeof formData] = ''
  })
  // 清除验证
  formRef.value?.clearValidate()
}

// 重置表单
const handleReset = () => {
  formRef.value?.resetFields()
  Object.keys(formData).forEach(key => {
    formData[key as keyof typeof formData] = ''
  })
}

// 预览配置
const handlePreview = async () => {
  try {
    await formRef.value?.validate()
    previewVisible.value = true
  } catch {
    ElMessage.warning('请先完善表单信息')
  }
}

// 提交表单
const handleSubmit = async () => {
  try {
    await formRef.value?.validate()
    previewVisible.value = true
  } catch {
    ElMessage.warning('请检查表单信息')
  }
}

// 构造 payload，兼容远程/本地/JSON 三种模式
const buildPayload = () => {
  if (serviceType.value === 'json') {
    // 用户直接贴 MCP JSON（必须是合法 JSON）
    try {
      const obj = JSON.parse(formData.jsonConfig)
      return obj
    } catch {
      ElMessage.error('JSON 配置不是合法的 JSON，请检查后重试')
      throw new Error('Invalid JSON config')
    }
  }
  if (serviceType.value === 'remote') {
    return {
      mcpServers: {
        [formData.name]: {
          url: formData.url
        }
      }
    }
  }
  // local
  return {
    mcpServers: {
      [formData.name]: {
        command: formData.command
      }
    }
  }
}

// 确认添加（真实 API）
const confirmAdd = async () => {
  submitLoading.value = true
  try {
    const payload = buildPayload()
    const res = await dashboardApi.addService(payload, 'auto')
    if (res?.success) {
      ElMessage.success(res?.message || '服务添加成功！')
      previewVisible.value = false
      // 询问是否跳转到服务列表
      ElMessageBox.confirm('是否跳转到服务列表查看？', '添加成功', {
        confirmButtonText: '查看列表',
        cancelButtonText: '继续添加',
        type: 'success'
      }).then(() => {
        router.push('/services')
      }).catch(() => {
        handleReset()
      })
    } else {
      ElMessage.error(res?.message || '添加服务失败')
    }
  } catch (error: any) {
    console.error(error)
    ElMessage.error(error?.message || '添加服务失败，请重试')
  } finally {
    submitLoading.value = false
  }
}
</script>

<style lang="scss" scoped>
.add-service-page {
  padding: 20px;
  
  .page-header {
    text-align: center;
    margin-bottom: 30px;
    
    .page-title {
      font-size: 24px;
      font-weight: 600;
      color: var(--el-text-color-primary);
      margin: 0 0 8px 0;
    }
    
    .page-subtitle {
      font-size: 14px;
      color: var(--el-text-color-secondary);
      margin: 0;
    }
  }
  
  .service-type-selector {
    display: flex;
    justify-content: center;
    margin-bottom: 40px;
    
    :deep(.el-radio-button__inner) {
      padding: 12px 24px;
      font-size: 16px;
    }
  }
  
  .form-container {
    max-width: 800px;
    margin: 0 auto;
    
    .form-section {
      .section-title {
        font-size: 18px;
        font-weight: 500;
        color: var(--el-text-color-primary);
        margin: 0 0 20px 0;
        padding-bottom: 10px;
        border-bottom: 2px solid var(--el-color-primary);
      }
    }
    
    .json-example {
      margin-top: 16px;
      
      .json-code {
        background: var(--el-fill-color-light);
        padding: 16px;
        border-radius: 6px;
        font-size: 12px;
        line-height: 1.5;
        overflow-x: auto;
      }
    }
    
    .form-actions {
      display: flex;
      justify-content: center;
      gap: 16px;
      margin-top: 40px;
      padding-top: 20px;
      border-top: 1px solid var(--el-border-color-light);
    }
  }
  
  .preview-content {
    background: var(--el-fill-color-light);
    padding: 16px;
    border-radius: 6px;
    font-size: 14px;
    line-height: 1.5;
    max-height: 400px;
    overflow-y: auto;
  }
  
  // 响应式设计
  @media (max-width: 768px) {
    padding: 12px;
    
    .form-container {
      max-width: 100%;
      
      :deep(.el-form-item__label) {
        width: 100px !important;
      }
    }
    
    .service-type-selector {
      :deep(.el-radio-button__inner) {
        padding: 8px 16px;
        font-size: 14px;
      }
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
