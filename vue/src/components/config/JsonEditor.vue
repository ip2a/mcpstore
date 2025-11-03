<template>
  <div class="json-editor-wrapper">
    <!-- 简化的文本编辑器 -->
    <el-input
      v-model="internalValue"
      type="textarea"
      :rows="Math.floor(height / 20)"
      placeholder="MCP JSON配置内容..."
      @focus="onFocus"
      @blur="onBlur"
      class="json-textarea"
      :class="{ 'has-error': !isValid && errorMessage }"
    />

    <!-- 编辑器工具栏 -->
    <div class="editor-toolbar">
      <div class="toolbar-left">
        <el-button
          size="small"
          :icon="Refresh"
          @click="formatCode"
          :disabled="!modelValue"
        >
          格式化
        </el-button>

        <el-button
          size="small"
          :icon="Check"
          @click="validateCode"
        >
          验证
        </el-button>

        <el-button
          size="small"
          :icon="CopyDocument"
          @click="copyCode"
          :disabled="!modelValue"
        >
          复制
        </el-button>
      </div>

      <div class="toolbar-right">
        <span class="status-info">
          <el-tag v-if="isValid && modelValue" type="success" size="small">语法正确</el-tag>
          <el-tag v-else-if="errorMessage" type="danger" size="small">语法错误</el-tag>
          <span class="char-count">字符: {{ modelValue.length }}</span>
        </span>
      </div>
    </div>

    <!-- 错误提示 -->
    <div v-if="errorMessage" class="error-panel">
      <el-alert
        :title="errorMessage"
        type="error"
        :closable="false"
        show-icon
      />
    </div>
  </div>
</template>

<script setup>
import { ref, computed, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { Refresh, Check, CopyDocument } from '@element-plus/icons-vue'

// Props
const props = defineProps({
  modelValue: {
    type: String,
    default: ''
  },
  height: {
    type: Number,
    default: 400
  },
  readonly: {
    type: Boolean,
    default: false
  }
})

// Emits
const emit = defineEmits(['update:modelValue', 'change', 'validate'])

// 响应式数据
const internalValue = ref(props.modelValue)
const isValid = ref(true)
const errorMessage = ref('')

// 计算属性
const charCount = computed(() => props.modelValue.length)

// 方法
const onFocus = () => {
  // 聚焦时验证
  validateCode()
}

const onBlur = () => {
  // 失焦时验证
  validateCode()
}

// 验证代码
const validateCode = () => {
  if (!props.modelValue.trim()) {
    isValid.value = true
    errorMessage.value = ''
    emit('validate', { isValid: true, error: null })
    return
  }

  try {
    const parsed = JSON.parse(props.modelValue)

    // 基本结构验证
    if (!parsed || typeof parsed !== 'object') {
      throw new Error('配置必须是一个有效的JSON对象')
    }

    if (!parsed.mcpServers || typeof parsed.mcpServers !== 'object') {
      throw new Error('配置必须包含 mcpServers 对象')
    }

    // 验证每个服务配置
    for (const [name, config] of Object.entries(parsed.mcpServers)) {
      if (!config || typeof config !== 'object') {
        throw new Error(`服务 "${name}" 配置必须是一个对象`)
      }

      if (!config.command && !config.url) {
        throw new Error(`服务 "${name}" 必须包含 command 或 url 字段`)
      }

      if (config.command && typeof config.command !== 'string') {
        throw new Error(`服务 "${name}" 的 command 字段必须是字符串`)
      }

      if (config.url && typeof config.url !== 'string') {
        throw new Error(`服务 "${name}" 的 url 字段必须是字符串`)
      }

      if (config.args && !Array.isArray(config.args)) {
        throw new Error(`服务 "${name}" 的 args 字段必须是数组`)
      }
    }

    isValid.value = true
    errorMessage.value = ''
    emit('validate', { isValid: true, error: null, data: parsed })

  } catch (error) {
    isValid.value = false
    errorMessage.value = error.message
    emit('validate', { isValid: false, error: error.message })
  }
}

// 格式化代码
const formatCode = () => {
  if (!props.modelValue) return

  try {
    const parsed = JSON.parse(props.modelValue)
    const formatted = JSON.stringify(parsed, null, 2)
    internalValue.value = formatted
    emit('update:modelValue', formatted)
    ElMessage.success('代码格式化完成')
  } catch (error) {
    ElMessage.error('格式化失败：JSON语法错误')
  }
}

// 复制代码
const copyCode = async () => {
  if (!props.modelValue) return

  try {
    await navigator.clipboard.writeText(props.modelValue)
    ElMessage.success('代码已复制到剪贴板')
  } catch (error) {
    ElMessage.error('复制失败')
  }
}

// 监听 props 变化
watch(() => props.modelValue, (newValue) => {
  console.log('🔍 JsonEditor props.modelValue 变化:', newValue?.substring(0, 100) + '...')
  if (internalValue.value !== newValue) {
    internalValue.value = newValue || ''
    console.log('🔍 JsonEditor internalValue 更新:', internalValue.value?.substring(0, 100) + '...')
    // 当内容变化时触发验证
    validateCode()
  }
}, { immediate: true })

// 监听内部值变化
watch(internalValue, (newValue) => {
  console.log('🔍 JsonEditor internalValue 变化，发送事件')
  emit('update:modelValue', newValue)
  emit('change', newValue)
  validateCode()
})

// 暴露方法
defineExpose({
  formatCode,
  validateCode,
  copyCode
})
</script>

<style scoped>
.json-editor-wrapper {
  border: 1px solid #dcdfe6;
  border-radius: 4px;
  overflow: hidden;
}

.json-textarea {
  font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
  font-size: 14px;
}

.json-textarea.has-error :deep(.el-textarea__inner) {
  border-color: #f56c6c;
  background-color: #fef0f0;
}

.json-textarea :deep(.el-textarea__inner) {
  border: none;
  border-radius: 0;
  resize: none;
  line-height: 1.5;
  padding: 12px;
}

.editor-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  background-color: #f5f7fa;
  border-top: 1px solid #dcdfe6;
}

.toolbar-left {
  display: flex;
  gap: 8px;
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: 12px;
}

.status-info {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: #606266;
}

.char-count {
  font-family: monospace;
}

.error-panel {
  padding: 8px 12px;
  background-color: #fef0f0;
  border-top: 1px solid #fbc4c4;
}
</style>
