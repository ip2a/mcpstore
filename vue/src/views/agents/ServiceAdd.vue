<template>
  <div class="service-add">
    <!-- 页面头部 -->
    <div class="page-header">
      <div class="header-left">
        <el-button 
          :icon="ArrowLeft" 
          @click="$router.back()"
          class="back-btn"
        >
          返回
        </el-button>
        <div class="title-section">
          <h2 class="page-title">添加服务</h2>
          <p class="page-description">
            {{ targetAgentId ? `为Agent ${targetAgentId} 添加服务` : '添加服务到新Agent' }}
          </p>
        </div>
      </div>
    </div>
    
    <!-- 服务配置表单 -->
    <el-card class="form-card">
      <template #header>
        <span>服务配置</span>
      </template>
      
      <el-form 
        ref="formRef" 
        :model="serviceForm" 
        :rules="formRules" 
        label-width="120px"
        class="service-form"
      >
        <!-- Agent ID -->
        <el-form-item label="Agent ID" prop="agentId">
          <el-input 
            v-model="serviceForm.agentId" 
            placeholder="输入Agent ID，如果不存在将自动创建"
            :disabled="!!targetAgentId"
          />
          <div class="form-tip">
            Agent ID用作服务的命名空间，如果Agent不存在将自动创建
          </div>
        </el-form-item>
        
        <!-- 服务类型 -->
        <el-form-item label="服务类型" prop="serviceType">
          <el-radio-group v-model="serviceForm.serviceType" @change="onServiceTypeChange">
            <el-radio value="remote">远程服务</el-radio>
            <el-radio value="local">本地服务</el-radio>
          </el-radio-group>
        </el-form-item>
        
        <!-- 服务名称 -->
        <el-form-item label="服务名称" prop="name">
          <el-input 
            v-model="serviceForm.name" 
            placeholder="输入服务名称"
          />
          <div class="form-tip">
            服务名称只能包含字母、数字、下划线和连字符
          </div>
        </el-form-item>
        
        <!-- 远程服务配置 -->
        <template v-if="serviceForm.serviceType === 'remote'">
          <el-form-item label="服务URL" prop="url">
            <el-input 
              v-model="serviceForm.url" 
              placeholder="输入服务URL，如: http://example.com/mcp"
            />
          </el-form-item>
          
          <el-form-item label="传输方式" prop="transport">
            <el-select v-model="serviceForm.transport" placeholder="选择传输方式">
              <el-option label="Streamable HTTP" value="streamable-http" />
              <el-option label="SSE" value="sse" />
            </el-select>
          </el-form-item>
        </template>
        
        <!-- 本地服务配置 -->
        <template v-if="serviceForm.serviceType === 'local'">
          <el-form-item label="命令" prop="command">
            <el-input 
              v-model="serviceForm.command" 
              placeholder="输入执行命令，如: python"
            />
          </el-form-item>
          
          <el-form-item label="参数" prop="args">
            <el-input 
              v-model="argsText" 
              type="textarea" 
              :rows="3"
              placeholder="输入命令参数，每行一个参数"
              @input="updateArgs"
            />
            <div class="form-tip">
              每行一个参数，例如：<br>
              ./server.py<br>
              --port<br>
              8080
            </div>
          </el-form-item>
          
          <el-form-item label="工作目录" prop="working_dir">
            <el-input 
              v-model="serviceForm.working_dir" 
              placeholder="输入工作目录路径（可选）"
            />
          </el-form-item>
          
          <el-form-item label="环境变量" prop="env">
            <div class="env-vars">
              <div 
                v-for="(envVar, index) in envVars" 
                :key="index" 
                class="env-var-item"
              >
                <el-input 
                  v-model="envVar.key" 
                  placeholder="变量名"
                  style="width: 40%"
                />
                <span class="env-separator">=</span>
                <el-input 
                  v-model="envVar.value" 
                  placeholder="变量值"
                  style="width: 40%"
                />
                <el-button 
                  type="danger" 
                  size="small" 
                  @click="removeEnvVar(index)"
                  style="width: 15%"
                >
                  删除
                </el-button>
              </div>
              <el-button 
                type="primary" 
                size="small" 
                @click="addEnvVar"
                class="add-env-btn"
              >
                添加环境变量
              </el-button>
            </div>
          </el-form-item>
        </template>
        
        <!-- 服务描述 -->
        <el-form-item label="服务描述" prop="description">
          <el-input 
            v-model="serviceForm.description" 
            type="textarea" 
            :rows="3"
            placeholder="输入服务描述（可选）"
          />
        </el-form-item>
        
        <!-- 操作按钮 -->
        <el-form-item>
          <el-button 
            type="primary" 
            @click="addService" 
            :loading="adding"
            size="large"
          >
            {{ adding ? '添加中...' : '添加服务' }}
          </el-button>
          <el-button @click="resetForm" size="large">
            重置
          </el-button>
        </el-form-item>
      </el-form>
    </el-card>
  </div>
</template>

<script setup>
import { ref, onMounted, computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { ArrowLeft } from '@element-plus/icons-vue'
import { useAgentsStore } from '@/stores/agents'
import { validateService } from '@/api/agents'

const route = useRoute()
const router = useRouter()
const agentsStore = useAgentsStore()

// 响应式数据
const formRef = ref()
const adding = ref(false)
const argsText = ref('')
const envVars = ref([{ key: '', value: '' }])

// 服务表单数据
const serviceForm = ref({
  agentId: '',
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

// 计算属性
const targetAgentId = computed(() => route.query.agentId)

// 表单验证规则
const formRules = {
  agentId: [
    { required: true, message: '请输入Agent ID', trigger: 'blur' },
    { pattern: /^[a-zA-Z0-9_-]+$/, message: 'Agent ID只能包含字母、数字、下划线和连字符', trigger: 'blur' }
  ],
  serviceType: [
    { required: true, message: '请选择服务类型', trigger: 'change' }
  ],
  name: [
    { required: true, message: '请输入服务名称', trigger: 'blur' },
    { pattern: /^[a-zA-Z0-9_-]+$/, message: '服务名称只能包含字母、数字、下划线和连字符', trigger: 'blur' }
  ],
  url: [
    { required: true, message: '请输入服务URL', trigger: 'blur' },
    { type: 'url', message: '请输入有效的URL', trigger: 'blur' }
  ],
  command: [
    { required: true, message: '请输入执行命令', trigger: 'blur' }
  ]
}

// 方法
const onServiceTypeChange = () => {
  // 清空相关字段
  if (serviceForm.value.serviceType === 'remote') {
    serviceForm.value.command = ''
    serviceForm.value.args = []
    serviceForm.value.working_dir = ''
    serviceForm.value.env = {}
  } else {
    serviceForm.value.url = ''
    serviceForm.value.transport = 'streamable-http'
  }
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

const addService = async () => {
  try {
    await formRef.value.validate()
    
    // 更新环境变量对象
    updateEnvObject()
    
    // 验证服务配置
    const validation = validateService(serviceForm.value)
    if (!validation.isValid) {
      ElMessage.error(validation.errors[0])
      return
    }
    
    adding.value = true
    
    // 构建服务配置
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
    
    // 添加服务
    const result = await agentsStore.addService(serviceForm.value.agentId, serviceConfig)
    
    if (result.success) {
      ElMessage.success('服务添加成功')
      router.push(`/agents/${serviceForm.value.agentId}/detail`)
    } else {
      ElMessage.error('服务添加失败: ' + result.error)
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
  serviceForm.value = {
    agentId: targetAgentId.value || '',
    serviceType: 'remote',
    name: '',
    url: '',
    transport: 'streamable-http',
    command: '',
    args: [],
    working_dir: '',
    env: {},
    description: ''
  }
  argsText.value = ''
  envVars.value = [{ key: '', value: '' }]
  formRef.value?.resetFields()
}

// 生命周期
onMounted(() => {
  if (targetAgentId.value) {
    serviceForm.value.agentId = targetAgentId.value
  }
})
</script>

<style lang="scss" scoped>
.service-add {
  .page-header {
    @include flex-between;
    margin-bottom: 20px;
    
    .header-left {
      display: flex;
      align-items: center;
      gap: 16px;
      
      .back-btn {
        padding: 8px 16px;
      }
      
      .title-section {
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
  }
  
  .form-card {
    .service-form {
      max-width: 800px;
      
      .form-tip {
        font-size: var(--font-size-xs);
        color: var(--text-secondary);
        margin-top: 4px;
        line-height: 1.4;
      }
      
      .env-vars {
        .env-var-item {
          display: flex;
          align-items: center;
          gap: 8px;
          margin-bottom: 8px;
          
          .env-separator {
            color: var(--text-secondary);
            font-weight: bold;
          }
        }
        
        .add-env-btn {
          margin-top: 8px;
        }
      }
    }
  }
}
</style>
