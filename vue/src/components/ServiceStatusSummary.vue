<template>
  <div class="service-status-summary">
    <!-- 状态统计卡片 -->
    <a-row :gutter="16" class="status-cards">
      <a-col :span="3">
        <a-card size="small" class="status-card total">
          <div class="status-number">{{ stats.total }}</div>
          <div class="status-label">总服务</div>
        </a-card>
      </a-col>
      
      <a-col :span="3">
        <a-card size="small" class="status-card healthy">
          <div class="status-number">{{ stats.healthy }}</div>
          <div class="status-label">健康</div>
        </a-card>
      </a-col>
      
      <a-col :span="3">
        <a-card size="small" class="status-card warning">
          <div class="status-number">{{ stats.warning }}</div>
          <div class="status-label">警告</div>
        </a-card>
      </a-col>
      
      <a-col :span="3">
        <a-card size="small" class="status-card reconnecting">
          <div class="status-number">{{ stats.reconnecting }}</div>
          <div class="status-label">重连中</div>
        </a-card>
      </a-col>
      
      <a-col :span="3">
        <a-card size="small" class="status-card unreachable">
          <div class="status-number">{{ stats.unreachable }}</div>
          <div class="status-label">无法访问</div>
        </a-card>
      </a-col>
      
      <a-col :span="3">
        <a-card size="small" class="status-card initializing">
          <div class="status-number">{{ stats.initializing }}</div>
          <div class="status-label">初始化中</div>
        </a-card>
      </a-col>
      
      <a-col :span="3">
        <a-card size="small" class="status-card disconnected">
          <div class="status-number">{{ stats.disconnected + stats.disconnecting }}</div>
          <div class="status-label">已断连</div>
        </a-card>
      </a-col>
    </a-row>

    <!-- 健康度指示器 -->
    <div class="health-indicator">
      <div class="health-bar">
        <div 
          class="health-progress healthy" 
          :style="{ width: healthPercentage + '%' }"
        ></div>
        <div 
          class="health-progress warning" 
          :style="{ width: warningPercentage + '%' }"
        ></div>
        <div 
          class="health-progress error" 
          :style="{ width: errorPercentage + '%' }"
        ></div>
      </div>
      <div class="health-text">
        <span class="health-score">系统健康度: {{ healthScore }}%</span>
        <span class="health-status" :class="healthStatusClass">{{ healthStatusText }}</span>
      </div>
    </div>

    <!-- 操作按钮 -->
    <div class="summary-actions">
      <a-button-group>
        <a-button 
          @click="handleRefreshAll"
          :loading="refreshingAll"
          icon="reload"
          type="primary"
        >
          刷新所有服务
        </a-button>
        <a-button 
          @click="handleRefreshStatus"
          :loading="refreshingStatus"
          icon="sync"
        >
          刷新状态
        </a-button>
        <a-button 
          @click="$emit('show-details')"
          icon="eye"
        >
          查看详情
        </a-button>
      </a-button-group>
    </div>
  </div>
</template>

<script>
import { storeMonitoringAPI } from '@/api/services'

// 简单的状态统计生成函数
const generateServiceStateStats = (services) => {
  const stats = {
    total: 0,
    active: 0,
    inactive: 0,
    error: 0
  }

  Object.values(services).forEach(service => {
    stats.total++
    if (service.status === 'active') {
      stats.active++
    } else if (service.status === 'error') {
      stats.error++
    } else {
      stats.inactive++
    }
  })

  return stats
}

export default {
  name: 'ServiceStatusSummary',
  props: {
    services: {
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
      refreshingAll: false,
      refreshingStatus: false
    }
  },
  
  computed: {
    stats() {
      return generateServiceStateStats(this.services)
    },
    
    healthPercentage() {
      if (this.stats.total === 0) return 0
      return Math.round((this.stats.healthy / this.stats.total) * 100)
    },
    
    warningPercentage() {
      if (this.stats.total === 0) return 0
      return Math.round(((this.stats.warning + this.stats.reconnecting) / this.stats.total) * 100)
    },
    
    errorPercentage() {
      if (this.stats.total === 0) return 0
      return Math.round(((this.stats.unreachable + this.stats.disconnected + this.stats.disconnecting) / this.stats.total) * 100)
    },
    
    healthScore() {
      if (this.stats.total === 0) return 100
      const healthyWeight = this.stats.healthy * 1.0
      const warningWeight = this.stats.warning * 0.7
      const reconnectingWeight = this.stats.reconnecting * 0.5
      const initializingWeight = this.stats.initializing * 0.8
      
      const totalWeight = healthyWeight + warningWeight + reconnectingWeight + initializingWeight
      return Math.round((totalWeight / this.stats.total) * 100)
    },
    
    healthStatusText() {
      if (this.healthScore >= 90) return '优秀'
      if (this.healthScore >= 80) return '良好'
      if (this.healthScore >= 70) return '一般'
      if (this.healthScore >= 60) return '较差'
      return '危险'
    },
    
    healthStatusClass() {
      if (this.healthScore >= 90) return 'excellent'
      if (this.healthScore >= 80) return 'good'
      if (this.healthScore >= 70) return 'fair'
      if (this.healthScore >= 60) return 'poor'
      return 'danger'
    }
  },
  
  methods: {
    async handleRefreshAll() {
      this.refreshingAll = true
      try {
        const response = await storeMonitoringAPI.refreshAllContent()
        
        if (response.success) {
          this.$message.success(`成功刷新 ${response.data.updated_services}/${response.data.total_services} 个服务`)
          this.$emit('refresh-success')
        } else {
          this.$message.error(`刷新失败: ${response.message}`)
        }
      } catch (error) {
        this.$message.error(`刷新失败: ${error.message}`)
      } finally {
        this.refreshingAll = false
      }
    },
    
    async handleRefreshStatus() {
      this.refreshingStatus = true
      try {
        this.$emit('refresh-status')
        this.$message.success('状态刷新完成')
      } catch (error) {
        this.$message.error(`状态刷新失败: ${error.message}`)
      } finally {
        this.refreshingStatus = false
      }
    }
  }
}
</script>

<style scoped>
.service-status-summary {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.status-cards {
  margin-bottom: 16px;
}

.status-card {
  text-align: center;
  cursor: pointer;
  transition: all 0.3s ease;
}

.status-card:hover {
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
}

.status-number {
  font-size: 24px;
  font-weight: bold;
  margin-bottom: 4px;
}

.status-label {
  font-size: 12px;
  color: #666;
}

/* 状态卡片颜色 */
.status-card.total .status-number { color: #1890ff; }
.status-card.healthy .status-number { color: #52c41a; }
.status-card.warning .status-number { color: #faad14; }
.status-card.reconnecting .status-number { color: #722ed1; }
.status-card.unreachable .status-number { color: #f5222d; }
.status-card.initializing .status-number { color: #1890ff; }
.status-card.disconnected .status-number { color: #8c8c8c; }

.health-indicator {
  background: #f5f5f5;
  border-radius: 8px;
  padding: 16px;
}

.health-bar {
  height: 8px;
  background: #e8e8e8;
  border-radius: 4px;
  overflow: hidden;
  display: flex;
  margin-bottom: 12px;
}

.health-progress {
  height: 100%;
  transition: width 0.3s ease;
}

.health-progress.healthy { background: #52c41a; }
.health-progress.warning { background: #faad14; }
.health-progress.error { background: #f5222d; }

.health-text {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.health-score {
  font-size: 16px;
  font-weight: bold;
}

.health-status {
  font-size: 14px;
  font-weight: 500;
  padding: 2px 8px;
  border-radius: 4px;
}

.health-status.excellent { background: #f6ffed; color: #52c41a; }
.health-status.good { background: #f9f0ff; color: #722ed1; }
.health-status.fair { background: #fffbe6; color: #faad14; }
.health-status.poor { background: #fff2e8; color: #fa8c16; }
.health-status.danger { background: #fff1f0; color: #f5222d; }

.summary-actions {
  display: flex;
  justify-content: center;
}
</style>
