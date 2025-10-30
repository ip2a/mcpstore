<template>
  <div class="tool-execute">
    <!-- 页面头部 -->
    <div class="page-header">
      <div class="header-left">
        <h2 class="page-title">
          <el-icon class="title-icon"><Tools /></el-icon>
          工具执行
        </h2>
        <p class="page-description">选择并执行MCP工具，查看实时结果</p>
      </div>
      <div class="header-right">
        <el-button @click="$router.back()">
          <el-icon><ArrowLeft /></el-icon>
          返回
        </el-button>
      </div>
    </div>

    <!-- 主要内容区域 -->
    <div class="main-content">
      <!-- 左侧：工具选择和配置 -->
      <div class="left-panel">
        <!-- 工具选择 -->
        <el-card class="selection-card">
          <template #header>
            <div class="card-header">
              <el-icon><Search /></el-icon>
              <span>选择工具</span>
            </div>
          </template>

          <div class="selection-form">
            <div class="form-group">
              <label class="form-label">服务</label>
              <el-select
                v-model="selectedService"
                placeholder="选择服务"
                @change="handleServiceChange"
                size="large"
                filterable
              >
                <el-option
                  v-for="serviceName in serviceNames"
                  :key="serviceName"
                  :label="serviceName"
                  :value="serviceName"
                >
                  <div class="service-option">
                    <span class="service-name">{{ serviceName }}</span>
                    <span class="tool-count">{{ getServiceToolCount(serviceName) }} 个工具</span>
                  </div>
                </el-option>
              </el-select>
            </div>

            <div class="form-group">
              <label class="form-label">工具</label>
              <el-select
                v-model="selectedTool"
                placeholder="选择工具"
                :disabled="!selectedService"
                @change="handleToolChange"
                size="large"
                filterable
              >
                <el-option
                  v-for="tool in availableTools"
                  :key="tool.name"
                  :label="tool.name"
                  :value="tool.name"
                >
                  <div class="tool-option">
                    <div class="tool-name">{{ tool.name }}</div>
                    <div class="tool-description">{{ tool.description || '暂无描述' }}</div>
                  </div>
                </el-option>
              </el-select>
            </div>
          </div>
        </el-card>

        <!-- 工具信息 -->
        <el-card v-if="currentTool" class="info-card">
          <template #header>
            <div class="card-header">
              <el-icon><InfoFilled /></el-icon>
              <span>工具信息</span>
            </div>
          </template>

          <div class="tool-info">
            <div class="info-item">
              <span class="info-label">工具名称</span>
              <span class="info-value">{{ currentTool.name }}</span>
            </div>
            <div class="info-item">
              <span class="info-label">所属服务</span>
              <span class="info-value">{{ currentTool.service }}</span>
            </div>
            <div class="info-item full-width">
              <span class="info-label">描述</span>
              <span class="info-value">{{ currentTool.description || '暂无描述' }}</span>
            </div>
            <div class="info-item" v-if="hasParameters">
              <span class="info-label">参数数量</span>
              <span class="info-value">{{ Object.keys(toolParameters).length }} 个</span>
            </div>
          </div>
        </el-card>
        <!-- 参数配置 -->
        <el-card v-if="currentTool" class="params-card">
          <template #header>
            <div class="card-header">
              <el-icon><Setting /></el-icon>
              <span>参数配置</span>
              <div class="header-actions" v-if="hasParameters">
                <el-button size="small" @click="resetParams" text>
                  <el-icon><RefreshLeft /></el-icon>
                  重置
                </el-button>
                <el-button size="small" @click="loadExample" text>
                  <el-icon><Star /></el-icon>
                  示例
                </el-button>
              </div>
            </div>
          </template>

          <div v-if="hasParameters" class="params-form">
            <el-form
              ref="paramsFormRef"
              :model="toolParams"
              :rules="paramRules"
              label-position="top"
            >
              <div class="params-grid">
                <el-form-item
                  v-for="(param, paramName) in toolParameters"
                  :key="paramName"
                  :label="paramName"
                  :prop="paramName"
                  class="param-item"
                >
                  <template #label>
                    <div class="param-label">
                      <span class="param-name">{{ paramName }}</span>
                      <el-tag
                        v-if="isRequired(paramName)"
                        size="small"
                        type="danger"
                      >
                        必需
                      </el-tag>
                      <el-tag
                        size="small"
                        type="info"
                      >
                        {{ param.type }}
                      </el-tag>
                    </div>
                  </template>
                  <!-- 字符串类型 -->
                  <el-input
                    v-if="param.type === 'string'"
                    v-model="toolParams[paramName]"
                    :placeholder="param.description || `请输入${paramName}`"
                    :type="param.format === 'password' ? 'password' : 'text'"
                    clearable
                    size="large"
                  />

                  <!-- 数字类型 -->
                  <el-input-number
                    v-else-if="param.type === 'number' || param.type === 'integer'"
                    v-model="toolParams[paramName]"
                    :min="param.minimum"
                    :max="param.maximum"
                    :step="param.type === 'integer' ? 1 : 0.1"
                    size="large"
                    style="width: 100%"
                  />

                  <!-- 布尔类型 -->
                  <div v-else-if="param.type === 'boolean'" class="boolean-input">
                    <el-switch
                      v-model="toolParams[paramName]"
                      size="large"
                      :active-text="toolParams[paramName] ? 'True' : 'False'"
                    />
                  </div>

                  <!-- 枚举类型 -->
                  <el-select
                    v-else-if="param.enum"
                    v-model="toolParams[paramName]"
                    placeholder="请选择"
                    size="large"
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
                        size="large"
                      />
                      <el-button
                        :icon="Delete"
                        @click="removeArrayItem(paramName, index)"
                        type="danger"
                        text
                        size="large"
                      />
                    </div>
                    <el-button
                      :icon="Plus"
                      @click="addArrayItem(paramName)"
                      type="primary"
                      text
                      size="large"
                      class="add-array-btn"
                    >
                      添加项
                    </el-button>
                  </div>

                  <!-- 对象类型 -->
                  <el-input
                    v-else
                    v-model="toolParams[paramName]"
                    type="textarea"
                    :rows="4"
                    placeholder="请输入JSON格式的对象"
                    size="large"
                  />

                  <div v-if="param.description" class="param-description">
                    <el-icon><Document /></el-icon>
                    {{ param.description }}
                  </div>
                </el-form-item>
              </div>
            </el-form>
          </div>

          <div v-else class="no-params">
            <el-icon class="no-params-icon"><InfoFilled /></el-icon>
            <span>此工具无需参数</span>
          </div>
        </el-card>
      </div>

      <!-- 右侧：执行控制和结果 -->
      <div class="right-panel">
        <!-- 执行控制 -->
        <el-card v-if="currentTool" class="execute-card">
          <template #header>
            <div class="card-header">
              <el-icon><VideoPlay /></el-icon>
              <span>执行工具</span>
            </div>
          </template>

          <div class="execute-section">
            <div class="execute-info">
              <div class="info-row">
                <span class="label">工具:</span>
                <span class="value">{{ currentTool.name }}</span>
              </div>
              <div class="info-row">
                <span class="label">服务:</span>
                <span class="value">{{ currentTool.service }}</span>
              </div>
              <div class="info-row" v-if="hasParameters">
                <span class="label">参数:</span>
                <span class="value">{{ Object.keys(toolParameters).length }} 个</span>
              </div>
            </div>

            <div class="execute-actions">
              <el-button
                type="primary"
                @click="executeTool"
                :loading="executing"
                size="large"
                :disabled="!canExecute"
                class="execute-btn"
              >
                <el-icon><VideoPlay /></el-icon>
                {{ executing ? '执行中...' : '执行工具' }}
              </el-button>
            </div>
          </div>
        </el-card>
        <!-- 执行结果 -->
        <el-card v-if="executionResult" class="result-card">
          <template #header>
            <div class="card-header">
              <div class="result-status">
                <el-icon v-if="executionResult.success"><CircleCheck /></el-icon>
                <el-icon v-else><CircleClose /></el-icon>
                <span>执行结果</span>
              </div>
              <el-tag
                :type="executionResult.success ? 'success' : 'danger'"
                size="large"
              >
                {{ executionResult.success ? '成功' : '失败' }}
              </el-tag>
            </div>
          </template>

          <div class="result-content">
            <!-- 执行信息 -->
            <div v-if="executionResult.execution_info" class="execution-summary">
              <div class="summary-item">
                <span class="summary-label">执行时间</span>
                <span class="summary-value">{{ executionResult.execution_info.duration_ms }}ms</span>
              </div>
              <div class="summary-item">
                <span class="summary-label">服务名称</span>
                <span class="summary-value">{{ executionResult.execution_info.service_name }}</span>
              </div>
              <div class="summary-item" v-if="executionResult.execution_info.trace_id">
                <span class="summary-label">追踪ID</span>
                <span class="summary-value">{{ executionResult.execution_info.trace_id }}</span>
              </div>
            </div>

            <!-- 结果数据 -->
            <div class="result-data">
              <div class="data-header">
                <h4>返回数据</h4>
                <div class="data-actions">
                  <el-button
                    :icon="DocumentCopy"
                    @click="copyResult"
                    size="small"
                    text
                  >
                    复制
                  </el-button>
                  <el-button
                    :icon="Download"
                    @click="downloadResult"
                    size="small"
                    text
                  >
                    下载
                  </el-button>
                </div>
              </div>
              <div class="result-display">
                <pre v-if="typeof executionResult.data === 'object'">{{ JSON.stringify(executionResult.data, null, 2) }}</pre>
                <div v-else class="text-result">{{ executionResult.data }}</div>
              </div>
            </div>

            <!-- 错误信息 -->
            <div v-if="!executionResult.success && executionResult.message" class="error-section">
              <h4>错误信息</h4>
              <el-alert
                :title="executionResult.message"
                type="error"
                :closable="false"
                show-icon
              />
            </div>
          </div>
        </el-card>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import { ElMessage } from 'element-plus'
import {
  VideoPlay, Plus, Delete, InfoFilled, DocumentCopy, Download,
  Tools, ArrowLeft, Search, Setting, RefreshLeft, Star, Document,
  CircleCheck, CircleClose
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
  const names = new Set(systemStore.tools.map(tool => tool.service))
  return Array.from(names).sort()
})

const availableTools = computed(() => {
  if (!selectedService.value) return []
  return systemStore.tools.filter(tool => tool.service === selectedService.value)
})

const currentTool = computed(() => {
  if (!selectedTool.value) return null
  return systemStore.tools.find(tool => tool.name === selectedTool.value)
})

const toolParameters = computed(() => {
  if (!currentTool.value?.input_schema?.properties) return {}
  return currentTool.value.input_schema.properties
})

const hasParameters = computed(() => {
  return Object.keys(toolParameters.value).length > 0
})

const paramRules = computed(() => {
  const rules = {}
  const required = currentTool.value?.input_schema?.required || []
  
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

// 新增：获取服务的工具数量
const getServiceToolCount = (serviceName) => {
  return systemStore.tools.filter(tool => tool.service === serviceName).length
}

// 新增：检查参数是否必需
const isRequired = (paramName) => {
  const required = currentTool.value?.input_schema?.required || []
  return required.includes(paramName)
}

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
      selectedService.value = tool.service
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
    margin-bottom: 24px;

    .header-left {
      .page-title {
        margin: 0 0 8px 0;
        font-size: 28px;
        font-weight: 600;
        display: flex;
        align-items: center;
        gap: 12px;

        .title-icon {
          font-size: 32px;
          color: var(--el-color-primary);
        }
      }

      .page-description {
        margin: 0;
        color: var(--el-text-color-secondary);
        font-size: 16px;
      }
    }
  }

  .main-content {
    display: grid;
    grid-template-columns: 1fr 400px;
    gap: 24px;
    align-items: start;
  }

  .left-panel {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .right-panel {
    display: flex;
    flex-direction: column;
    gap: 20px;
    position: sticky;
    top: 20px;
  }
  // 卡片头部样式
  .card-header {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 500;

    .header-actions {
      margin-left: auto;
      display: flex;
      gap: 8px;
    }
  }

  // 选择卡片样式
  .selection-card {
    .selection-form {
      display: flex;
      flex-direction: column;
      gap: 20px;

      .form-group {
        .form-label {
          display: block;
          margin-bottom: 8px;
          font-weight: 500;
          color: var(--el-text-color-primary);
        }
      }
    }

    .service-option {
      display: flex;
      justify-content: space-between;
      align-items: center;

      .service-name {
        font-weight: 500;
      }

      .tool-count {
        font-size: 12px;
        color: var(--el-text-color-secondary);
      }
    }

    .tool-option {
      .tool-name {
        font-weight: 500;
        margin-bottom: 4px;
      }

      .tool-description {
        font-size: 12px;
        color: var(--el-text-color-secondary);
        line-height: 1.4;
      }
    }
  }

  // 工具信息卡片样式
  .info-card {
    .tool-info {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 16px;

      .info-item {
        display: flex;
        flex-direction: column;
        gap: 4px;

        &.full-width {
          grid-column: 1 / -1;
        }

        .info-label {
          font-size: 12px;
          color: var(--el-text-color-secondary);
          font-weight: 500;
        }

        .info-value {
          color: var(--el-text-color-primary);
          font-weight: 500;
        }
      }
    }
  }
  
  // 参数表单样式
  .params-card {
    .params-form {
      .params-grid {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
        gap: 20px;
      }

      .param-item {
        .param-label {
          display: flex;
          align-items: center;
          gap: 8px;
          margin-bottom: 8px;

          .param-name {
            font-weight: 500;
            color: var(--el-text-color-primary);
          }
        }

        .boolean-input {
          display: flex;
          align-items: center;
          gap: 12px;
        }

        .array-input {
          .array-item {
            display: flex;
            align-items: center;
            gap: 8px;
            margin-bottom: 12px;
          }

          .add-array-btn {
            width: 100%;
            border: 2px dashed var(--el-border-color);
            background: transparent;

            &:hover {
              border-color: var(--el-color-primary);
              background: var(--el-color-primary-light-9);
            }
          }
        }

        .param-description {
          font-size: 12px;
          color: var(--el-text-color-secondary);
          margin-top: 8px;
          padding: 8px 12px;
          background: var(--el-fill-color-lighter);
          border-radius: 4px;
          display: flex;
          align-items: center;
          gap: 6px;
        }
      }
    }
  }
  
  .no-params {
    text-align: center;
    padding: 60px 20px;
    color: var(--el-text-color-secondary);

    .no-params-icon {
      font-size: 48px;
      margin-bottom: 12px;
      display: block;
      color: var(--el-color-info);
    }
  }

  // 执行控制样式
  .execute-card {
    .execute-section {
      .execute-info {
        margin-bottom: 20px;
        padding: 16px;
        background: var(--el-fill-color-lighter);
        border-radius: 8px;

        .info-row {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 8px;

          &:last-child {
            margin-bottom: 0;
          }

          .label {
            font-size: 14px;
            color: var(--el-text-color-secondary);
          }

          .value {
            font-weight: 500;
            color: var(--el-text-color-primary);
          }
        }
      }

      .execute-actions {
        text-align: center;

        .execute-btn {
          width: 100%;
          height: 48px;
          font-size: 16px;
          font-weight: 500;
        }
      }
    }
  }
  
  // 结果显示样式
  .result-card {
    .card-header {
      .result-status {
        display: flex;
        align-items: center;
        gap: 8px;

        .el-icon {
          font-size: 20px;

          &:first-child {
            color: var(--el-color-success);
          }

          &:first-child:has(+ span) {
            color: var(--el-color-danger);
          }
        }
      }
    }

    .result-content {
      .execution-summary {
        display: flex;
        gap: 20px;
        margin-bottom: 24px;
        padding: 16px;
        background: var(--el-fill-color-lighter);
        border-radius: 8px;

        .summary-item {
          display: flex;
          flex-direction: column;
          gap: 4px;

          .summary-label {
            font-size: 12px;
            color: var(--el-text-color-secondary);
          }

          .summary-value {
            font-weight: 500;
            color: var(--el-text-color-primary);
          }
        }
      }

      .result-data {
        margin-bottom: 20px;

        .data-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 12px;

          h4 {
            margin: 0;
            color: var(--el-text-color-primary);
            font-size: 16px;
          }

          .data-actions {
            display: flex;
            gap: 8px;
          }
        }

        .result-display {
          background: var(--el-fill-color-blank);
          border: 1px solid var(--el-border-color);
          border-radius: 8px;
          padding: 20px;

          pre {
            margin: 0;
            font-family: 'JetBrains Mono', 'Fira Code', 'Consolas', 'Monaco', monospace;
            font-size: 13px;
            line-height: 1.6;
            max-height: 500px;
            overflow-y: auto;
            color: var(--el-text-color-primary);
          }

          .text-result {
            font-family: 'JetBrains Mono', 'Fira Code', 'Consolas', 'Monaco', monospace;
            font-size: 13px;
            line-height: 1.6;
            white-space: pre-wrap;
            word-break: break-word;
            color: var(--el-text-color-primary);
          }
        }
      }

      .error-section {
        h4 {
          margin-bottom: 12px;
          color: var(--el-color-danger);
          font-size: 16px;
        }
      }
    }
  }
}

// 响应式适配
@include respond-to(lg) {
  .tool-execute {
    .main-content {
      grid-template-columns: 1fr 350px;
    }
  }
}

@include respond-to(md) {
  .tool-execute {
    .main-content {
      grid-template-columns: 1fr;
      gap: 20px;
    }

    .right-panel {
      position: static;
    }

    .params-card .params-form .params-grid {
      grid-template-columns: 1fr;
    }

    .info-card .tool-info {
      grid-template-columns: 1fr;
    }
  }
}

@include respond-to(sm) {
  .tool-execute {
    .page-header {
      flex-direction: column;
      align-items: flex-start;
      gap: 16px;

      .header-left .page-title {
        font-size: 24px;

        .title-icon {
          font-size: 28px;
        }
      }
    }

    .selection-card .selection-form {
      gap: 16px;
    }

    .result-card .result-content .execution-summary {
      flex-direction: column;
      gap: 12px;
    }
  }
}

@include respond-to(xs) {
  .tool-execute {
    .page-header .header-left .page-title {
      font-size: 20px;

      .title-icon {
        font-size: 24px;
      }
    }

    .card-header .header-actions {
      flex-direction: column;
      gap: 4px;
    }

    .result-card .result-content .data-header {
      flex-direction: column;
      align-items: flex-start;
      gap: 8px;
    }
  }
}
</style>
