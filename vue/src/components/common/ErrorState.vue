<template>
  <div class="error-state">
    <div class="error-content">
      <el-icon class="error-icon" :size="iconSize">
        <component :is="iconComponent" />
      </el-icon>
      
      <h3 class="error-title">{{ title }}</h3>
      <p class="error-description">{{ description }}</p>
      
      <div class="error-actions" v-if="showRetry || showGoHome">
        <el-button 
          v-if="showRetry"
          type="primary" 
          :icon="Refresh" 
          @click="handleRetry"
          :loading="retrying"
        >
          重试
        </el-button>
        
        <el-button 
          v-if="showGoHome"
          :icon="HomeFilled" 
          @click="goHome"
        >
          返回首页
        </el-button>
      </div>
      
      <div class="error-details" v-if="showDetails && errorDetails">
        <el-collapse>
          <el-collapse-item title="错误详情" name="details">
            <pre class="error-detail-text">{{ errorDetails }}</pre>
          </el-collapse-item>
        </el-collapse>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue'
import { useRouter } from 'vue-router'
import {
  Refresh,
  HomeFilled,
  Connection,
  Warning,
  CircleCloseFilled,
  QuestionFilled
} from '@element-plus/icons-vue'

const props = defineProps({
  type: {
    type: String,
    default: 'network', // network, server, unknown, permission
    validator: (value) => ['network', 'server', 'unknown', 'permission'].includes(value)
  },
  title: {
    type: String,
    default: ''
  },
  description: {
    type: String,
    default: ''
  },
  showRetry: {
    type: Boolean,
    default: true
  },
  showGoHome: {
    type: Boolean,
    default: false
  },
  showDetails: {
    type: Boolean,
    default: false
  },
  errorDetails: {
    type: String,
    default: ''
  },
  iconSize: {
    type: [String, Number],
    default: 64
  }
})

const emit = defineEmits(['retry'])

const router = useRouter()
const retrying = ref(false)

// 根据错误类型计算图标和默认文案
const iconComponent = computed(() => {
  const icons = {
    network: Connection,
    server: Warning,
    permission: CircleCloseFilled,
    unknown: QuestionFilled
  }
  return icons[props.type] || QuestionFilled
})

const computedTitle = computed(() => {
  if (props.title) return props.title
  
  const titles = {
    network: '网络连接失败',
    server: '服务器错误',
    permission: '权限不足',
    unknown: '未知错误'
  }
  return titles[props.type] || '出现错误'
})

const computedDescription = computed(() => {
  if (props.description) return props.description
  
  const descriptions = {
    network: '无法连接到后端服务，请检查网络连接或服务器状态',
    server: '服务器内部错误，请稍后重试',
    permission: '您没有权限访问此资源',
    unknown: '发生了未知错误，请联系管理员'
  }
  return descriptions[props.type] || '请稍后重试'
})

const handleRetry = async () => {
  retrying.value = true
  try {
    emit('retry')
  } finally {
    // 延迟重置状态，给用户反馈
    setTimeout(() => {
      retrying.value = false
    }, 1000)
  }
}

const goHome = () => {
  router.push('/')
}
</script>

<style lang="scss" scoped>
.error-state {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 400px;
  padding: 40px 20px;
}

.error-content {
  text-align: center;
  max-width: 500px;
}

.error-icon {
  color: var(--el-color-danger);
  margin-bottom: 24px;
  opacity: 0.8;
}

.error-title {
  font-size: 24px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  margin: 0 0 12px 0;
}

.error-description {
  font-size: 16px;
  color: var(--el-text-color-regular);
  line-height: 1.6;
  margin: 0 0 32px 0;
}

.error-actions {
  display: flex;
  gap: 16px;
  justify-content: center;
  margin-bottom: 24px;
}

.error-details {
  text-align: left;
  margin-top: 24px;
}

.error-detail-text {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  background: var(--el-fill-color-light);
  padding: 12px;
  border-radius: 4px;
  white-space: pre-wrap;
  word-break: break-all;
  max-height: 200px;
  overflow-y: auto;
}

// 响应式适配
@media (max-width: 768px) {
  .error-state {
    min-height: 300px;
    padding: 20px;
  }
  
  .error-title {
    font-size: 20px;
  }
  
  .error-description {
    font-size: 14px;
  }
  
  .error-actions {
    flex-direction: column;
    align-items: center;
    
    .el-button {
      width: 120px;
    }
  }
}
</style>
