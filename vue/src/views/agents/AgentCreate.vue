<template>
  <div class="agent-create">
    <!-- 页面头部 -->
    <div class="page-header">
      <div class="header-left">
        <h2 class="page-title">创建Agent</h2>
        <p class="page-description">创建新的Agent实例</p>
      </div>
      <div class="header-right">
        <el-button @click="$router.back()">
          返回
        </el-button>
      </div>
    </div>
    
    <!-- 创建表单 -->
    <el-card class="form-card">
      <template #header>
        <span>Agent配置</span>
      </template>
      
      <el-form
        ref="formRef"
        :model="agentForm"
        :rules="formRules"
        label-width="120px"
      >
        <el-form-item label="Agent名称" prop="name">
          <el-input 
            v-model="agentForm.name" 
            placeholder="请输入Agent名称"
            clearable
          />
        </el-form-item>
        
        <el-form-item label="Agent ID" prop="id">
          <el-input 
            v-model="agentForm.id" 
            placeholder="留空自动生成"
            clearable
          />
          <div class="form-tip">
            Agent的唯一标识符，留空将自动生成
          </div>
        </el-form-item>
        
        <el-form-item label="描述">
          <el-input 
            v-model="agentForm.description" 
            type="textarea"
            :rows="3"
            placeholder="请输入Agent描述"
          />
        </el-form-item>
        
        <el-form-item label="初始服务">
          <el-select 
            v-model="agentForm.services" 
            multiple
            placeholder="选择要分配给Agent的服务"
            style="width: 100%"
          >
            <el-option 
              v-for="service in availableServices"
              :key="service.name"
              :label="service.name"
              :value="service.name"
            />
          </el-select>
          <div class="form-tip">
            可以稍后在Agent管理页面中添加更多服务
          </div>
        </el-form-item>
        
        <el-form-item label="配置选项">
          <el-checkbox-group v-model="agentForm.options">
            <el-checkbox label="auto_start">自动启动</el-checkbox>
            <el-checkbox label="enable_logging">启用日志</el-checkbox>
            <el-checkbox label="enable_monitoring">启用监控</el-checkbox>
          </el-checkbox-group>
        </el-form-item>
      </el-form>
    </el-card>
    
    <!-- 预览 -->
    <el-card class="preview-card">
      <template #header>
        <span>配置预览</span>
      </template>
      
      <el-alert
        title="Agent配置预览"
        type="info"
        :closable="false"
        style="margin-bottom: 16px"
      >
        <pre>{{ configPreview }}</pre>
      </el-alert>
      
      <div class="submit-actions">
        <el-button 
          type="primary" 
          @click="createAgent"
          :loading="creating"
          size="large"
        >
          创建Agent
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
import { ref, computed, reactive, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import { ElMessage } from 'element-plus'

const router = useRouter()
const systemStore = useSystemStore()

// 响应式数据
const creating = ref(false)
const formRef = ref()

// 表单数据
const agentForm = reactive({
  name: '',
  id: '',
  description: '',
  services: [],
  options: ['enable_logging']
})

// 表单验证规则
const formRules = {
  name: [
    { required: true, message: '请输入Agent名称', trigger: 'blur' }
  ]
}

// 计算属性
const availableServices = computed(() => systemStore.services)

const configPreview = computed(() => {
  const config = {
    name: agentForm.name,
    id: agentForm.id || 'auto_generated',
    description: agentForm.description,
    services: agentForm.services,
    options: {
      auto_start: agentForm.options.includes('auto_start'),
      enable_logging: agentForm.options.includes('enable_logging'),
      enable_monitoring: agentForm.options.includes('enable_monitoring')
    }
  }
  
  return JSON.stringify(config, null, 2)
})

// 方法
const createAgent = async () => {
  try {
    await formRef.value.validate()
    
    creating.value = true
    
    // 生成Agent ID（如果未提供）
    if (!agentForm.id) {
      agentForm.id = `agent_${Date.now()}`
    }
    
    // 这里应该调用创建Agent的API
    await new Promise(resolve => setTimeout(resolve, 2000))
    
    ElMessage.success('Agent创建成功')
    router.push('/agents/list')
  } catch (error) {
    if (error !== 'validation failed') {
      ElMessage.error('Agent创建失败: ' + (error.message || error))
    }
  } finally {
    creating.value = false
  }
}

const resetForm = () => {
  Object.assign(agentForm, {
    name: '',
    id: '',
    description: '',
    services: [],
    options: ['enable_logging']
  })
  formRef.value?.resetFields()
}

// 生命周期
onMounted(async () => {
  await systemStore.fetchServices()
})
</script>

<style lang="scss" scoped>
.agent-create {
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
  
  .form-card,
  .preview-card {
    margin-bottom: 20px;
  }
  
  .form-tip {
    font-size: var(--font-size-xs);
    color: var(--text-secondary);
    margin-top: 4px;
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
  .agent-create {
    .page-header {
      flex-direction: column;
      align-items: flex-start;
      gap: 16px;
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
