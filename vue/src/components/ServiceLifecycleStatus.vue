<template>
  <div class="service-lifecycle-status">
    <!-- 服务状态标签 -->
    <a-tag 
      :color="getServiceStateColor(status)" 
      :icon="getServiceStateIcon(status)"
      class="status-tag"
    >
      <a-icon :type="getServiceStateIcon(status)" />
      {{ getServiceStateText(status) }}
    </a-tag>

    <!-- 详细信息（可选展开） -->
    <div v-if="showDetails" class="status-details">
      <a-descriptions size="small" :column="2" bordered>
        <a-descriptions-item label="服务名称">
          {{ serviceName }}
        </a-descriptions-item>
        <a-descriptions-item label="当前状态">
          <a-tag :color="getServiceStateColor(status)">
            {{ getServiceStateText(status) }}
          </a-tag>
        </a-descriptions-item>
        <a-descriptions-item label="响应时间">
          {{ formatResponseTime(responseTime) }}
        </a-descriptions-item>
        <a-descriptions-item label="连续失败">
          {{ consecutiveFailures || 0 }} 次
        </a-descriptions-item>
        <a-descriptions-item label="连续成功">
          {{ consecutiveSuccesses || 0 }} 次
        </a-descriptions-item>
        <a-descriptions-item label="重连尝试">
          {{ reconnectAttempts || 0 }} 次
        </a-descriptions-item>
        <a-descriptions-item label="状态进入时间">
          {{ formatISOTime(stateEnteredTime) }}
        </a-descriptions-item>
        <a-descriptions-item label="最后检查时间">
          {{ formatTimestamp(lastCheckTime) }}
        </a-descriptions-item>
        <a-descriptions-item v-if="nextRetryTime" label="下次重试时间">
          {{ formatISOTime(nextRetryTime) }}
        </a-descriptions-item>
        <a-descriptions-item v-if="errorMessage" label="错误信息" :span="2">
          <a-alert :message="errorMessage" type="error" show-icon />
        </a-descriptions-item>
      </a-descriptions>
    </div>

    <!-- 操作按钮 -->
    <div v-if="showActions" class="status-actions">
      <a-button-group size="small">
        <a-button 
          @click="handleRefreshContent"
          :loading="refreshing"
          icon="reload"
        >
          刷新内容
        </a-button>
        <a-button 
          @click="handleHealthCheck"
          :loading="checking"
          icon="heart"
        >
          健康检查
        </a-button>
        <a-button 
          v-if="isServiceAvailable(status)"
          @click="handleDisconnect"
          :loading="disconnecting"
          icon="disconnect"
          type="danger"
        >
          断开连接
        </a-button>
      </a-button-group>
    </div>
  </div>
</template>

<script>
import { api } from '@/api'

// 简化的状态处理函数
const getServiceStateText = (state) => {
  const stateMap = {
    'active': '运行中',
    'inactive': '未激活',
    'error': '错误',
    'loading': '加载中'
  }
  return stateMap[state] || '未知'
}

const getServiceStateColor = (state) => {
  const colorMap = {
    'active': '#52c41a',
    'inactive': '#d9d9d9',
    'error': '#ff4d4f',
    'loading': '#1890ff'
  }
  return colorMap[state] || '#d9d9d9'
}

const getServiceStateIcon = (state) => {
  const iconMap = {
    'active': 'check-circle',
    'inactive': 'pause-circle',
    'error': 'close-circle',
    'loading': 'loading'
  }
  return iconMap[state] || 'question-circle'
}

const isServiceAvailable = (state) => {
  return state === 'active'
}

const formatResponseTime = (time) => {
  if (!time) return 'N/A'
  return `${time}ms`
}

const formatTimestamp = (timestamp) => {
  if (!timestamp) return 'N/A'
  return new Date(timestamp).toLocaleString()
}

const formatISOTime = (isoTime) => {
  if (!isoTime) return 'N/A'
  return new Date(isoTime).toLocaleString()
}

export default {
  name: 'ServiceLifecycleStatus',
  props: {
    // 基本信息
    serviceName: {
      type: String,
      required: true
    },
    status: {
      type: String,
      required: true,
      validator: (value) => Object.values(SERVICE_LIFECYCLE_STATE).includes(value)
    },
    agentId: {
      type: String,
      default: null
    },
    
    // 状态详情
    responseTime: {
      type: Number,
      default: 0
    },
    lastCheckTime: {
      type: Number,
      default: 0
    },
    consecutiveFailures: {
      type: Number,
      default: 0
    },
    consecutiveSuccesses: {
      type: Number,
      default: 0
    },
    reconnectAttempts: {
      type: Number,
      default: 0
    },
    stateEnteredTime: {
      type: String,
      default: null
    },
    nextRetryTime: {
      type: String,
      default: null
    },
    errorMessage: {
      type: String,
      default: null
    },
    
    // 显示控制
    showDetails: {
      type: Boolean,
      default: false
    },
    showActions: {
      type: Boolean,
      default: false
    }
  },
  
  data() {
    return {
      refreshing: false,
      checking: false,
      disconnecting: false
    }
  },
  
  methods: {
    // 工具函数
    getServiceStateText,
    getServiceStateColor,
    getServiceStateIcon,
    isServiceAvailable,
    formatResponseTime,
    formatTimestamp,
    formatISOTime,
    
    // 操作方法
    async handleRefreshContent() {
      this.refreshing = true
      try {
        const response = await api.monitoring.refreshServiceTools(
          this.serviceName, 
          this.agentId
        )
        
        if (response.success) {
          this.$message.success(`服务 ${this.serviceName} 内容刷新成功`)
          this.$emit('refresh-success', response.data)
        } else {
          this.$message.error(`内容刷新失败: ${response.message}`)
        }
      } catch (error) {
        this.$message.error(`内容刷新失败: ${error.message}`)
      } finally {
        this.refreshing = false
      }
    },
    
    async handleHealthCheck() {
      this.checking = true
      try {
        const response = await api.monitoring.triggerHealthCheck(this.serviceName)
        
        if (response.success) {
          this.$message.success(`服务 ${this.serviceName} 健康检查完成`)
          this.$emit('health-check-success', response.data)
        } else {
          this.$message.error(`健康检查失败: ${response.message}`)
        }
      } catch (error) {
        this.$message.error(`健康检查失败: ${error.message}`)
      } finally {
        this.checking = false
      }
    },
    
    async handleDisconnect() {
      this.$confirm({
        title: '确认断开连接',
        content: `确定要断开服务 "${this.serviceName}" 的连接吗？`,
        okText: '确认',
        cancelText: '取消',
        onOk: async () => {
          this.disconnecting = true
          try {
            const response = await api.monitoring.gracefulDisconnect(
              this.serviceName,
              this.agentId,
              'user_requested'
            )
            
            if (response.success) {
              this.$message.success(`服务 ${this.serviceName} 断开连接成功`)
              this.$emit('disconnect-success', response.data)
            } else {
              this.$message.error(`断开连接失败: ${response.message}`)
            }
          } catch (error) {
            this.$message.error(`断开连接失败: ${error.message}`)
          } finally {
            this.disconnecting = false
          }
        }
      })
    }
  }
}
</script>

<style scoped>
.service-lifecycle-status {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.status-tag {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-weight: 500;
}

.status-details {
  margin-top: 8px;
}

.status-actions {
  margin-top: 8px;
}

.status-actions .ant-btn-group {
  display: flex;
  gap: 4px;
}
</style>
