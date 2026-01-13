<template>
  <div class="service-status-summary">
    <!-- 状态统计卡片 -->
    <a-row
      :gutter="16"
      class="status-cards"
    >
      <a-col :span="3">
        <a-card
          size="small"
          class="status-card total"
        >
          <div class="status-number">
            {{ stats.total }}
          </div>
          <div class="status-label">
            总服务
          </div>
        </a-card>
      </a-col>
      
      <a-col :span="3">
        <a-card
          size="small"
          class="status-card ready"
        >
          <div class="status-number">
            {{ stats.ready }}
          </div>
          <div class="status-label">
            已就绪
          </div>
        </a-card>
      </a-col>
      
      <a-col :span="3">
        <a-card
          size="small"
          class="status-card healthy"
        >
          <div class="status-number">
            {{ stats.healthy }}
          </div>
          <div class="status-label">
            健康
          </div>
        </a-card>
      </a-col>
      
      <a-col :span="3">
        <a-card
          size="small"
          class="status-card degraded"
        >
          <div class="status-number">
            {{ stats.degraded }}
          </div>
          <div class="status-label">
            性能下降
          </div>
        </a-card>
      </a-col>
      
      <a-col :span="3">
        <a-card
          size="small"
          class="status-card half-open"
        >
          <div class="status-number">
            {{ stats.half_open }}
          </div>
          <div class="status-label">
            半开试探
          </div>
        </a-card>
      </a-col>
      
      <a-col :span="3">
        <a-card
          size="small"
          class="status-card circuit-open"
        >
          <div class="status-number">
            {{ stats.circuit_open }}
          </div>
          <div class="status-label">
            已熔断
          </div>
        </a-card>
      </a-col>
      
      <a-col :span="3">
        <a-card
          size="small"
          class="status-card disconnected"
        >
          <div class="status-number">
            {{ stats.disconnected }}
          </div>
          <div class="status-label">
            已断连
          </div>
        </a-card>
      </a-col>
    </a-row>

    <!-- 健康度指示器 -->
    <div class="health-indicator">
      <div class="health-bar">
        <div 
          class="health-progress healthy" 
          :style="{ width: healthPercentage + '%' }"
        />
        <div 
          class="health-progress warning" 
          :style="{ width: warningPercentage + '%' }"
        />
        <div 
          class="health-progress error" 
          :style="{ width: errorPercentage + '%' }"
        />
      </div>
      <div class="health-text">
        <span class="health-score">系统健康度: {{ healthScore }}%</span>
        <span
          class="health-status"
          :class="healthStatusClass"
        >{{ healthStatusText }}</span>
      </div>
    </div>

    <!-- 操作按钮 -->
    <div class="summary-actions">
      <a-button-group>
        <a-button 
          :loading="refreshingAll"
          icon="reload"
          type="primary"
          @click="handleRefreshAll"
        >
          刷新所有服务
        </a-button>
        <a-button 
          :loading="refreshingStatus"
          icon="sync"
          @click="handleRefreshStatus"
        >
          刷新状态
        </a-button>
        <a-button 
          icon="eye"
          @click="$emit('show-details')"
        >
          查看详情
        </a-button>
      </a-button-group>
    </div>
  </div>
</template>

<script>
import { api } from '@/api'

// 简单的状态统计生成函数
const generateServiceStateStats = (services) => {
  const stats = {
    total: 0,
    ready: 0,
    healthy: 0,
    degraded: 0,
    half_open: 0,
    circuit_open: 0,
    disconnected: 0
  }

  Object.values(services).forEach(service => {
    stats.total++
    const status = service.status || 'unknown'
    if (status in stats) stats[status]++
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
      return Math.round(((this.stats.degraded + this.stats.half_open) / this.stats.total) * 100)
    },
    
    errorPercentage() {
      if (this.stats.total === 0) return 0
      return Math.round(((this.stats.circuit_open + this.stats.disconnected) / this.stats.total) * 100)
    },
    
    healthScore() {
      if (this.stats.total === 0) return 100
      const healthyWeight = this.stats.healthy * 1.0
      const readyWeight = this.stats.ready * 0.85
      const degradedWeight = this.stats.degraded * 0.6
      const halfOpenWeight = this.stats.half_open * 0.45
      
      const totalWeight = healthyWeight + readyWeight + degradedWeight + halfOpenWeight
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
        const response = await api.monitoring.refreshAllTools()
        
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
.status-card.ready .status-number { color: #0ea5e9; }
.status-card.healthy .status-number { color: #52c41a; }
.status-card.degraded .status-number { color: #faad14; }
.status-card.half-open .status-number { color: #f97316; }
.status-card.circuit-open .status-number { color: #f5222d; }
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
