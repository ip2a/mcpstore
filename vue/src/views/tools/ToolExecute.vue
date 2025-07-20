<template>
  <div class="tool-execute">
    <!-- 页面头部 -->
    <div class="page-header">
      <div class="header-left">
        <h2 class="page-title">工具执行</h2>
        <p class="page-description">选择并执行MCP工具</p>
      </div>
      <div class="header-right">
        <el-button @click="$router.back()">
          返回
        </el-button>
      </div>
    </div>
    
    <!-- 工具选择 -->
    <el-card class="tool-selection-card">
      <template #header>
        <span>选择工具</span>
      </template>
      
      <el-row :gutter="20">
        <el-col :xs="24" :sm="12">
          <el-select 
            v-model="selectedService" 
            placeholder="选择服务"
            @change="handleServiceChange"
            style="width: 100%"
          >
            <el-option 
              v-for="serviceName in serviceNames"
              :key="serviceName"
              :label="serviceName"
              :value="serviceName"
            />
          </el-select>
        </el-col>
        <el-col :xs="24" :sm="12">
          <el-select 
            v-model="selectedTool" 
            placeholder="选择工具"
            :disabled="!selectedService"
            @change="handleToolChange"
            style="width: 100%"
          >
            <el-option 
              v-for="tool in availableTools"
              :key="tool.name"
              :label="tool.name"
              :value="tool.name"
            >
              <div class="tool-option">
                <span>{{ tool.name }}</span>
                <span class="tool-description">{{ tool.description || '暂无描述' }}</span>
              </div>
            </el-option>
          </el-select>
        </el-col>
      </el-row>
    </el-card>
    
    <!-- 工具信息 -->
    <el-card v-if="currentTool" class="tool-info-card">
      <template #header>
        <span>工具信息</span>
      </template>
      
      <el-descriptions :column="2" border>
        <el-descriptions-item label="工具名称">
          {{ currentTool.name }}
        </el-descriptions-item>
        <el-descriptions-item label="所属服务">
          {{ currentTool.service_name }}
        </el-descriptions-item>
        <el-descriptions-item label="描述" :span="2">
          {{ currentTool.description || '暂无描述' }}
        </el-descriptions-item>
      </el-descriptions>
    </el-card>
    
    <!-- 参数配置 -->
    <el-card v-if="currentTool" class="params-card">
      <template #header>
        <span>参数配置</span>
      </template>
      
      <div v-if="hasParameters" class="params-form">
        <el-form 
          ref="paramsFormRef"
          :model="toolParams"
          :rules="paramRules"
          label-width="120px"
        >
          <el-form-item 
            v-for="(param, paramName) in toolParameters"
            :key="paramName"
            :label="paramName"
            :prop="paramName"
          >
            <!-- 字符串类型 -->
            <el-input
              v-if="param.type === 'string'"
              v-model="toolParams[paramName]"
              :placeholder="param.description || `请输入${paramName}`"
              :type="param.format === 'password' ? 'password' : 'text'"
              clearable
            />
            
            <!-- 数字类型 -->
            <el-input-number
              v-else-if="param.type === 'number' || param.type === 'integer'"
              v-model="toolParams[paramName]"
              :min="param.minimum"
              :max="param.maximum"
              :step="param.type === 'integer' ? 1 : 0.1"
              style="width: 100%"
            />
            
            <!-- 布尔类型 -->
            <el-switch
              v-else-if="param.type === 'boolean'"
              v-model="toolParams[paramName]"
            />
            
            <!-- 枚举类型 -->
            <el-select
              v-else-if="param.enum"
              v-model="toolParams[paramName]"
              placeholder="请选择"
              style="width: 100%"
            >
              <el-option 
                v-for="option in param.enum"
                :key="option"
                :label="option"
                :value="option"
              />
            </el-select>
            
            <!-- 数组类型 -->
            <div v-else-if="param.type === 'array'" class="array-input">
              <div 
                v-for="(item, index) in toolParams[paramName]" 
                :key="index"
                class="array-item"
              >
                <el-input 
                  v-model="toolParams[paramName][index]" 
                  placeholder="数组项"
                  style="width: 80%"
                />
                <el-button 
                  :icon="Delete" 
                  @click="removeArrayItem(paramName, index)"
                  type="danger"
                  text
                />
              </div>
              <el-button 
                :icon="Plus" 
                @click="addArrayItem(paramName)"
                type="primary"
                text
              >
                添加项
              </el-button>
            </div>
            
            <!-- 对象类型 -->
            <el-input
              v-else
              v-model="toolParams[paramName]"
              type="textarea"
              :rows="3"
              placeholder="请输入JSON格式的对象"
            />
            
            <div v-if="param.description" class="param-description">
              {{ param.description }}
            </div>
          </el-form-item>
        </el-form>
      </div>
      
      <div v-else class="no-params">
        <el-icon class="no-params-icon"><InfoFilled /></el-icon>
        <span>此工具无需参数</span>
      </div>
    </el-card>
    
    <!-- 执行控制 -->
    <el-card v-if="currentTool" class="execute-card">
      <template #header>
        <span>执行工具</span>
      </template>
      
      <div class="execute-actions">
        <el-button 
          type="primary" 
          @click="executeTool"
          :loading="executing"
          size="large"
          :disabled="!canExecute"
        >
          <el-icon><VideoPlay /></el-icon>
          执行工具
        </el-button>
        
        <el-button 
          @click="resetParams"
          size="large"
        >
          重置参数
        </el-button>
        
        <el-button 
          @click="loadExample"
          size="large"
          v-if="hasParameters"
        >
          加载示例
        </el-button>
      </div>
    </el-card>
    
    <!-- 执行结果 -->
    <el-card v-if="executionResult" class="result-card">
      <template #header>
        <div class="result-header">
          <span>执行结果</span>
          <el-tag 
            :type="executionResult.success ? 'success' : 'danger'"
            size="small"
          >
            {{ executionResult.success ? '成功' : '失败' }}
          </el-tag>
        </div>
      </template>
      
      <div class="result-content">
        <!-- 执行信息 -->
        <div v-if="executionResult.execution_info" class="execution-info">
          <h4>执行信息</h4>
          <el-descriptions :column="3" size="small">
            <el-descriptions-item label="执行时间">
              {{ executionResult.execution_info.duration_ms }}ms
            </el-descriptions-item>
            <el-descriptions-item label="服务名称">
              {{ executionResult.execution_info.service_name }}
            </el-descriptions-item>
            <el-descriptions-item label="追踪ID">
              {{ executionResult.execution_info.trace_id }}
            </el-descriptions-item>
          </el-descriptions>
        </div>
        
        <!-- 结果数据 -->
        <div class="result-data">
          <h4>返回数据</h4>
          <div class="result-display">
            <pre v-if="typeof executionResult.data === 'object'">{{ JSON.stringify(executionResult.data, null, 2) }}</pre>
            <div v-else class="text-result">{{ executionResult.data }}</div>
          </div>
        </div>
        
        <!-- 错误信息 -->
        <div v-if="!executionResult.success && executionResult.message" class="error-info">
          <h4>错误信息</h4>
          <el-alert 
            :title="executionResult.message"
            type="error"
            :closable="false"
          />
        </div>
      </div>
      
      <div class="result-actions">
        <el-button 
          :icon="DocumentCopy" 
          @click="copyResult"
        >
          复制结果
        </el-button>
        <el-button 
          :icon="Download" 
          @click="downloadResult"
        >
          下载结果
        </el-button>
      </div>
    </el-card>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import { ElMessage } from 'element-plus'
import {
  VideoPlay, Plus, Delete, InfoFilled, DocumentCopy, Download
} from '@element-plus/icons-vue'

const route = useRoute()
const systemStore = useSystemStore()

// 响应式数据
const selectedService = ref('')
const selectedTool = ref('')
const toolParams = ref({})
const executing = ref(false)
const executionResult = ref(null)
const paramsFormRef = ref()

// 计算属性
const serviceNames = computed(() => {
  const names = new Set(systemStore.tools.map(tool => tool.service_name))
  return Array.from(names).sort()
})

const availableTools = computed(() => {
  if (!selectedService.value) return []
  return systemStore.tools.filter(tool => tool.service_name === selectedService.value)
})

const currentTool = computed(() => {
  if (!selectedTool.value) return null
  return systemStore.tools.find(tool => tool.name === selectedTool.value)
})

const toolParameters = computed(() => {
  if (!currentTool.value?.inputSchema?.properties) return {}
  return currentTool.value.inputSchema.properties
})

const hasParameters = computed(() => {
  return Object.keys(toolParameters.value).length > 0
})

const paramRules = computed(() => {
  const rules = {}
  const required = currentTool.value?.inputSchema?.required || []
  
  Object.keys(toolParameters.value).forEach(paramName => {
    if (required.includes(paramName)) {
      rules[paramName] = [
        { required: true, message: `${paramName} 是必需参数`, trigger: 'blur' }
      ]
    }
  })
  
  return rules
})

const canExecute = computed(() => {
  return currentTool.value && !executing.value
})

// 方法
const handleServiceChange = () => {
  selectedTool.value = ''
  toolParams.value = {}
  executionResult.value = null
}

const handleToolChange = () => {
  initializeParams()
  executionResult.value = null
}

const initializeParams = () => {
  const params = {}
  Object.keys(toolParameters.value).forEach(paramName => {
    const param = toolParameters.value[paramName]
    if (param.type === 'array') {
      params[paramName] = []
    } else if (param.type === 'boolean') {
      params[paramName] = false
    } else if (param.default !== undefined) {
      params[paramName] = param.default
    } else {
      params[paramName] = ''
    }
  })
  toolParams.value = params
}

const addArrayItem = (paramName) => {
  if (!toolParams.value[paramName]) {
    toolParams.value[paramName] = []
  }
  toolParams.value[paramName].push('')
}

const removeArrayItem = (paramName, index) => {
  toolParams.value[paramName].splice(index, 1)
}

const resetParams = () => {
  initializeParams()
  executionResult.value = null
}

const loadExample = () => {
  // 加载示例参数
  Object.keys(toolParameters.value).forEach(paramName => {
    const param = toolParameters.value[paramName]
    if (param.example !== undefined) {
      toolParams.value[paramName] = param.example
    } else if (param.type === 'string') {
      toolParams.value[paramName] = `示例${paramName}`
    } else if (param.type === 'number') {
      toolParams.value[paramName] = 123
    } else if (param.type === 'boolean') {
      toolParams.value[paramName] = true
    }
  })
}

const executeTool = async () => {
  if (!currentTool.value) return
  
  try {
    // 验证表单
    if (hasParameters.value) {
      await paramsFormRef.value.validate()
    }
    
    executing.value = true
    
    // 处理参数
    const processedParams = {}
    Object.keys(toolParams.value).forEach(key => {
      const value = toolParams.value[key]
      const param = toolParameters.value[key]
      
      if (param.type === 'object' && typeof value === 'string') {
        try {
          processedParams[key] = JSON.parse(value)
        } catch {
          processedParams[key] = value
        }
      } else {
        processedParams[key] = value
      }
    })
    
    // 执行工具
    const result = await systemStore.executeToolAction(selectedTool.value, processedParams)
    executionResult.value = result
    
    ElMessage.success('工具执行成功')
  } catch (error) {
    ElMessage.error('工具执行失败: ' + (error.message || error))
    executionResult.value = {
      success: false,
      message: error.message || error,
      data: null
    }
  } finally {
    executing.value = false
  }
}

const copyResult = () => {
  if (!executionResult.value) return
  
  const text = typeof executionResult.value.data === 'object' 
    ? JSON.stringify(executionResult.value.data, null, 2)
    : executionResult.value.data
    
  navigator.clipboard.writeText(text).then(() => {
    ElMessage.success('结果已复制到剪贴板')
  }).catch(() => {
    ElMessage.error('复制失败')
  })
}

const downloadResult = () => {
  if (!executionResult.value) return
  
  const text = typeof executionResult.value.data === 'object' 
    ? JSON.stringify(executionResult.value.data, null, 2)
    : executionResult.value.data
    
  const blob = new Blob([text], { type: 'text/plain' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `${selectedTool.value}-result.txt`
  a.click()
  URL.revokeObjectURL(url)
}

// 监听路由参数
watch(() => route.query.tool, (toolName) => {
  if (toolName) {
    const tool = systemStore.tools.find(t => t.name === toolName)
    if (tool) {
      selectedService.value = tool.service_name
      selectedTool.value = tool.name
    }
  }
}, { immediate: true })

// 生命周期
onMounted(async () => {
  await systemStore.fetchTools()
})
</script>

<style lang="scss" scoped>
.tool-execute {
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
  
  .tool-selection-card,
  .tool-info-card,
  .params-card,
  .execute-card,
  .result-card {
    margin-bottom: 20px;
  }
  
  .tool-option {
    display: flex;
    flex-direction: column;
    
    .tool-description {
      font-size: var(--font-size-xs);
      color: var(--text-secondary);
      margin-top: 2px;
    }
  }
  
  .params-form {
    .array-input {
      .array-item {
        display: flex;
        align-items: center;
        gap: 8px;
        margin-bottom: 8px;
      }
    }
    
    .param-description {
      font-size: var(--font-size-xs);
      color: var(--text-secondary);
      margin-top: 4px;
    }
  }
  
  .no-params {
    text-align: center;
    padding: 40px;
    color: var(--text-secondary);
    
    .no-params-icon {
      font-size: 32px;
      margin-bottom: 8px;
      display: block;
    }
  }
  
  .execute-actions {
    display: flex;
    gap: 12px;
    justify-content: center;
  }
  
  .result-card {
    .result-header {
      @include flex-between;
    }
    
    .result-content {
      .execution-info,
      .result-data,
      .error-info {
        margin-bottom: 20px;
        
        h4 {
          margin-bottom: 12px;
          color: var(--text-primary);
        }
      }
      
      .result-display {
        background: var(--bg-color-page);
        border-radius: var(--border-radius-base);
        padding: 16px;
        
        pre {
          margin: 0;
          font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
          font-size: var(--font-size-sm);
          line-height: 1.4;
          max-height: 400px;
          overflow-y: auto;
        }
        
        .text-result {
          font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
          font-size: var(--font-size-sm);
          line-height: 1.4;
          white-space: pre-wrap;
          word-break: break-all;
        }
      }
    }
    
    .result-actions {
      margin-top: 16px;
      display: flex;
      gap: 8px;
    }
  }
}

// 响应式适配
@include respond-to(xs) {
  .tool-execute {
    .page-header {
      flex-direction: column;
      align-items: flex-start;
      gap: 16px;
    }
    
    .execute-actions {
      flex-direction: column;
      
      .el-button {
        width: 100%;
      }
    }
    
    .result-actions {
      flex-direction: column;
      
      .el-button {
        width: 100%;
      }
    }
  }
}
</style>
