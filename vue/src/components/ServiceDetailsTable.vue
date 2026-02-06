<template>
  <div class="service-details-table">
    <a-table
      :columns="columns"
      :data-source="tableData"
      :loading="loading"
      :pagination="pagination"
      :scroll="{ x: 1200 }"
      row-key="key"
      size="middle"
    >
      <!-- 服务名称列 -->
      <template #serviceName="text, record">
        <div class="service-name-cell">
          <strong>{{ record.serviceName }}</strong>
          <div class="agent-info">
            {{ record.agentId }}
          </div>
        </div>
      </template>

      <!-- 状态列 -->
      <template #status="text, record">
        <ServiceLifecycleStatus
          :service-name="record.serviceName"
          :status="record.status"
          :agent-id="record.agentId"
          :response-time="record.responseTime"
          :last-check-time="record.lastCheckTime"
          :consecutive-failures="record.consecutiveFailures"
          :consecutive-successes="record.consecutiveSuccesses"
          :reconnect-attempts="record.reconnectAttempts"
          :state-entered-time="record.stateEnteredTime"
          :next-retry-time="record.nextRetryTime"
          :error-message="record.errorMessage"
          @refresh-success="handleRefreshSuccess"
          @health-check-success="handleHealthCheckSuccess"
          @disconnect-success="handleDisconnectSuccess"
        />
      </template>

      <!-- 响应时间列 -->
      <template #responseTime="text">
        <span :class="getResponseTimeClass(text)">
          {{ formatResponseTime(text) }}
        </span>
      </template>

      <!-- 最后检查时间列 -->
      <template #lastCheckTime="text">
        <div class="time-cell">
          <div>{{ formatTimestamp(text) }}</div>
          <div class="time-ago">
            {{ getTimeDifference(text * 1000) }}
          </div>
        </div>
      </template>

      <!-- 失败/成功次数列 -->
      <template #failures="text, record">
        <div class="failure-success-cell">
          <a-badge 
            :count="record.consecutiveFailures" 
            :number-style="{ backgroundColor: record.consecutiveFailures > 0 ? '#f5222d' : '#52c41a' }"
          />
          <span class="success-count">{{ record.consecutiveSuccesses }}✓</span>
        </div>
      </template>

      <!-- 内容快照列 -->
      <template #contentSnapshot="text, record">
        <div class="content-snapshot-cell">
          <a-tooltip title="工具数量">
            <a-tag color="blue">
              {{ record.toolsCount || 0 }} 工具
            </a-tag>
          </a-tooltip>
          <div class="snapshot-time">
            {{ formatISOTime(record.lastUpdated) }}
          </div>
        </div>
      </template>

      <!-- 操作列 -->
      <template #actions="text, record">
        <a-button-group size="small">
          <a-tooltip title="查看详情">
            <a-button 
              icon="eye" 
              @click="showServiceDetails(record)"
            />
          </a-tooltip>
          <a-tooltip title="刷新内容">
            <a-button 
              icon="reload" 
              :loading="record.refreshing"
              @click="refreshServiceContent(record)"
            />
          </a-tooltip>
          <a-tooltip title="健康检查">
            <a-button 
              icon="heart" 
              :loading="record.checking"
              @click="triggerHealthCheck(record)"
            />
          </a-tooltip>
          <a-dropdown v-if="isServiceAvailable(record.status)">
            <a-button icon="more" />
            <template #overlay>
              <a-menu>
                <a-menu-item @click="gracefulDisconnect(record)">
                  <a-icon type="disconnect" />
                  断开连接
                </a-menu-item>
                <a-menu-item @click="viewContentSnapshot(record)">
                  <a-icon type="camera" />
                  查看快照
                </a-menu-item>
              </a-menu>
            </template>
          </a-dropdown>
        </a-button-group>
      </template>
    </a-table>

    <!-- 服务详情抽屉 -->
    <a-drawer
      title="服务详情"
      :visible="detailsVisible"
      :width="600"
      @close="detailsVisible = false"
    >
      <ServiceLifecycleStatus
        v-if="selectedService"
        :service-name="selectedService.serviceName"
        :status="selectedService.status"
        :agent-id="selectedService.agentId"
        :response-time="selectedService.responseTime"
        :last-check-time="selectedService.lastCheckTime"
        :consecutive-failures="selectedService.consecutiveFailures"
        :consecutive-successes="selectedService.consecutiveSuccesses"
        :reconnect-attempts="selectedService.reconnectAttempts"
        :state-entered-time="selectedService.stateEnteredTime"
        :next-retry-time="selectedService.nextRetryTime"
        :error-message="selectedService.errorMessage"
        :show-details="true"
        :show-actions="true"
        @refresh-success="handleRefreshSuccess"
        @health-check-success="handleHealthCheckSuccess"
        @disconnect-success="handleDisconnectSuccess"
      />
    </a-drawer>
  </div>
</template>

<script>
import ServiceLifecycleStatus from './ServiceLifecycleStatus.vue'
import { api } from '@/api'
import { isServiceAvailable } from '@/utils/serviceStatus'

// 简化的工具函数
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

const getTimeDifference = (startTime, endTime) => {
  if (!startTime || !endTime) return 'N/A'
  const diff = new Date(endTime) - new Date(startTime)
  return `${Math.round(diff / 1000)}s`
}

export default {
  name: 'ServiceDetailsTable',
  components: {
    ServiceLifecycleStatus
  },
  props: {
    services: {
      type: Object,
      default: () => ({})
    },
    contentSnapshots: {
      type: Object,
      default: () => ({})
    },
    loading: {
      type: Boolean,
      default: false
    }
  },
  
  data() {
    return {
      detailsVisible: false,
      selectedService: null,
      pagination: {
        pageSize: 20,
        showSizeChanger: true,
        showQuickJumper: true,
        showTotal: (total) => `共 ${total} 个服务`
      }
    }
  },
  
  computed: {
    columns() {
      return [
        {
          title: '服务名称',
          dataIndex: 'serviceName',
          key: 'serviceName',
          width: 200,
          fixed: 'left',
          scopedSlots: { customRender: 'serviceName' }
        },
        {
          title: '状态',
          dataIndex: 'status',
          key: 'status',
          width: 150,
          scopedSlots: { customRender: 'status' }
        },
        {
          title: '响应时间',
          dataIndex: 'responseTime',
          key: 'responseTime',
          width: 100,
          scopedSlots: { customRender: 'responseTime' },
          sorter: (a, b) => a.responseTime - b.responseTime
        },
        {
          title: '最后检查',
          dataIndex: 'lastCheckTime',
          key: 'lastCheckTime',
          width: 150,
          scopedSlots: { customRender: 'lastCheckTime' },
          sorter: (a, b) => a.lastCheckTime - b.lastCheckTime
        },
        {
          title: '失败/成功',
          dataIndex: 'consecutiveFailures',
          key: 'failures',
          width: 100,
          scopedSlots: { customRender: 'failures' }
        },
        {
          title: '内容快照',
          dataIndex: 'contentSnapshot',
          key: 'contentSnapshot',
          width: 120,
          scopedSlots: { customRender: 'contentSnapshot' }
        },
        {
          title: '操作',
          key: 'actions',
          width: 150,
          fixed: 'right',
          scopedSlots: { customRender: 'actions' }
        }
      ]
    },
    
    tableData() {
      const data = []
      
      Object.entries(this.services).forEach(([key, service]) => {
        const [agentId, serviceName] = key.includes(':') ? key.split(':') : ['global_agent_store', key]
        const snapshot = this.contentSnapshots[key] || {}
        
        data.push({
          key,
          serviceName: service.service_name || serviceName,
          agentId,
          status: service.status,
          responseTime: service.response_time || 0,
          lastCheckTime: service.last_check_time || 0,
          consecutiveFailures: service.consecutive_failures || 0,
          consecutiveSuccesses: service.consecutive_successes || 0,
          reconnectAttempts: service.reconnect_attempts || 0,
          stateEnteredTime: service.state_entered_time,
          nextRetryTime: service.next_retry_time,
          errorMessage: service.error_message,
          toolsCount: snapshot.tools_count || 0,
          lastUpdated: snapshot.last_updated,
          refreshing: false,
          checking: false
        })
      })
      
      return data
    }
  },
  
  methods: {
    // 工具函数
    formatResponseTime,
    formatTimestamp,
    formatISOTime,
    getTimeDifference,
    isServiceAvailable,
    
    // 响应时间样式
    getResponseTimeClass(responseTime) {
      if (!responseTime || responseTime === 0) return 'response-time-unknown'
      if (responseTime < 0.5) return 'response-time-fast'
      if (responseTime < 2) return 'response-time-normal'
      return 'response-time-slow'
    },
    
    // 操作方法
    showServiceDetails(record) {
      this.selectedService = record
      this.detailsVisible = true
    },
    
    async refreshServiceContent(record) {
      this.$set(record, 'refreshing', true)
      try {
        const response = await api.monitoring.refreshServiceTools(
          record.serviceName,
          record.agentId
        )
        
        if (response.success) {
          this.$message.success(`服务 ${record.serviceName} 内容刷新成功`)
          this.$emit('refresh-success')
        } else {
          this.$message.error(`内容刷新失败: ${response.message}`)
        }
      } catch (error) {
        this.$message.error(`内容刷新失败: ${error.message}`)
      } finally {
        this.$set(record, 'refreshing', false)
      }
    },
    
    async triggerHealthCheck(record) {
      this.$set(record, 'checking', true)
      try {
        const response = await api.monitoring.triggerHealthCheck(record.serviceName)
        
        if (response.success) {
          this.$message.success(`服务 ${record.serviceName} 健康检查完成`)
          this.$emit('health-check-success')
        } else {
          this.$message.error(`健康检查失败: ${response.message}`)
        }
      } catch (error) {
        this.$message.error(`健康检查失败: ${error.message}`)
      } finally {
        this.$set(record, 'checking', false)
      }
    },
    
    gracefulDisconnect(record) {
      this.$confirm({
        title: '确认断开连接',
        content: `确定要断开服务 "${record.serviceName}" 的连接吗？`,
        okText: '确认',
        cancelText: '取消',
        onOk: async () => {
          try {
            const response = await api.monitoring.gracefulDisconnect(
              record.serviceName,
              record.agentId,
              'user_requested'
            )
            
            if (response.success) {
              this.$message.success(`服务 ${record.serviceName} 断开连接成功`)
              this.$emit('disconnect-success')
            } else {
              this.$message.error(`断开连接失败: ${response.message}`)
            }
          } catch (error) {
            this.$message.error(`断开连接失败: ${error.message}`)
          }
        }
      })
    },
    
    viewContentSnapshot(record) {
      // 实现查看内容快照的逻辑
      this.$message.info('查看内容快照功能开发中...')
    },
    
    // 事件处理
    handleRefreshSuccess() {
      this.$emit('refresh-success')
    },
    
    handleHealthCheckSuccess() {
      this.$emit('health-check-success')
    },
    
    handleDisconnectSuccess() {
      this.$emit('disconnect-success')
    }
  }
}
</script>

<style scoped>
.service-details-table {
  background: white;
  border-radius: 8px;
}

.service-name-cell {
  display: flex;
  flex-direction: column;
}

.agent-info {
  font-size: 12px;
  color: #666;
  margin-top: 2px;
}

.time-cell {
  display: flex;
  flex-direction: column;
}

.time-ago {
  font-size: 12px;
  color: #999;
  margin-top: 2px;
}

.failure-success-cell {
  display: flex;
  align-items: center;
  gap: 8px;
}

.success-count {
  color: #52c41a;
  font-size: 12px;
}

.content-snapshot-cell {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.snapshot-time {
  font-size: 12px;
  color: #666;
}

/* 响应时间样式 */
.response-time-fast { color: #52c41a; }
.response-time-normal { color: #1890ff; }
.response-time-slow { color: #faad14; }
.response-time-unknown { color: #8c8c8c; }
</style>
