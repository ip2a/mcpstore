<template>
  <div class="agent-list">
    <!-- 页面头部 -->
    <div class="page-header">
      <div class="header-left">
        <h2 class="page-title">Agent管理</h2>
        <p class="page-description">管理Agent和对应的MCP服务</p>
      </div>
      <div class="header-right">
        <el-button
          type="primary"
          :icon="Plus"
          @click="$router.push('/for_store/add_service')"
        >
          添加服务
        </el-button>
        <el-button
          :icon="Refresh"
          @click="refreshAgents"
          :loading="agentsStore.loading"
        >
          刷新
        </el-button>
      </div>
    </div>
    
    <!-- Agent列表 -->
    <el-card class="agents-card">
      <div v-if="agentsStore.agents.length === 0 && !agentsStore.loading" class="empty-container">
        <el-icon class="empty-icon"><User /></el-icon>
        <div class="empty-text">暂无Agent</div>
        <div class="empty-description">还没有任何Agent，通过添加服务来创建第一个Agent</div>
        <el-button
          type="primary"
          @click="$router.push('/for_store/add_service')"
        >
          添加第一个服务
        </el-button>
      </div>

      <div v-else class="agents-grid">
        <div
          v-for="agent in agentsStore.agents"
          :key="agent.id"
          class="agent-card"
          @click="viewAgentDetails(agent)"
        >
          <div class="agent-header">
            <div class="agent-info">
              <div class="agent-name">{{ agent.name }}</div>
              <div class="agent-id">ID: {{ agent.id }}</div>
            </div>
            <el-tag
              :type="getStatusType(agent.status)"
              size="small"
            >
              {{ getStatusText(agent.status) }}
            </el-tag>
          </div>
          
          <div class="agent-body">
            <div class="agent-description">
              {{ agent.description || '暂无描述' }}
            </div>

            <div class="agent-stats">
              <div class="stat-item">
                <span class="stat-label">服务数:</span>
                <span class="stat-value">{{ agent.services || 0 }}</span>
              </div>
              <div class="stat-item">
                <span class="stat-label">工具数:</span>
                <span class="stat-value">{{ agent.tools || 0 }}</span>
              </div>
              <div class="stat-item">
                <span class="stat-label">健康服务:</span>
                <span class="stat-value">{{ agent.healthy_services || 0 }}</span>
              </div>
              <div class="stat-item">
                <span class="stat-label">不健康服务:</span>
                <span class="stat-value">{{ agent.unhealthy_services || 0 }}</span>
              </div>
              <div class="stat-item">
                <span class="stat-label">最后活动:</span>
                <span class="stat-value">{{ formatTime(agent.last_activity) }}</span>
              </div>
            </div>
          </div>

          <div class="agent-footer">
            <el-button-group>
              <el-button
                size="small"
                @click.stop="addServiceToAgent(agent)"
                type="primary"
              >
                添加服务
              </el-button>
              <el-button
                size="small"
                @click.stop="manageAgent(agent)"
              >
                管理服务
              </el-button>
              <el-button
                size="small"
                type="danger"
                @click.stop="resetAgent(agent)"
              >
                重置
              </el-button>
            </el-button-group>
          </div>
        </div>
      </div>
    </el-card>
  </div>
</template>

<script setup>
import { onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import dayjs from 'dayjs'
import { Plus, Refresh, User } from '@element-plus/icons-vue'
import { useAgentsStore } from '@/stores/agents'
const router = useRouter()
const agentsStore = useAgentsStore()

// Agent状态映射
const AGENT_STATUS_MAP = {
  active: '活跃',
  inactive: '非活跃',
  partial: '部分可用',
  error: '错误'
}

const AGENT_STATUS_COLORS = {
  active: 'success',
  inactive: 'info',
  partial: 'warning',
  error: 'danger'
}

// 状态处理函数
const getStatusType = (status) => {
  return AGENT_STATUS_COLORS[status] || 'info'
}

const getStatusText = (status) => {
  return AGENT_STATUS_MAP[status] || '未知'
}

// 方法
const refreshAgents = async () => {
  try {
    await agentsStore.fetchAgents()
    ElMessage.success('Agent列表刷新成功')
  } catch (error) {
    console.error('刷新Agent列表失败:', error)
    ElMessage.error('刷新失败: ' + (error.message || error))
  }
}

const viewAgentDetails = (agent) => {
  router.push({ path: '/for_store/list_agents', query: { agentId: agent.id } })
}

const addServiceToAgent = (agent) => {
  router.push(`/for_agent/${agent.id}/add_service`)
}

const manageAgent = (agent) => {
  router.push({ path: '/for_store/list_agents', query: { agentId: agent.id } })
}

const resetAgent = async (agent) => {
  try {
    await ElMessageBox.confirm(
      `确定要重置Agent "${agent.name}" 吗？这将删除该Agent下的所有服务。`,
      '重置确认',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )

    const result = await agentsStore.resetAgentConfig(agent.id)
    if (result.success) {
      ElMessage.success(`Agent ${agent.name} 重置成功`)
    } else {
      ElMessage.error(`Agent ${agent.name} 重置失败: ${result.error}`)
    }
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error(`Agent ${agent.name} 重置失败`)
    }
  }
}

const formatTime = (time) => {
  if (!time) return '未知'
  return dayjs(time).format('YYYY-MM-DD HH:mm')
}

// 生命周期
onMounted(async () => {
  await refreshAgents()
})
</script>

<style lang="scss" scoped>
.agent-list {
  width: 92%;
  margin: 0 auto;
  max-width: none;
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
    
    .header-right {
      display: flex;
      gap: 12px;
    }
  }
  
  .agents-card {
    .agents-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
      gap: 20px;
      
      .agent-card {
        @include card-shadow;
        padding: 16px;
        border-radius: var(--border-radius-base);
        cursor: pointer;
        transition: all 0.3s ease;

        &:hover {
          transform: translateY(-2px);
          box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
        }
        
        .agent-header {
          @include flex-between;
          margin-bottom: 12px;
          
          .agent-info {
            .agent-name {
              font-weight: var(--font-weight-medium);
              margin-bottom: 4px;
            }
            
            .agent-id {
              font-size: var(--font-size-xs);
              color: var(--text-secondary);
            }
          }
        }
        
        .agent-body {
          margin-bottom: 16px;
          
          .agent-description {
            color: var(--text-regular);
            margin-bottom: 12px;
            font-size: var(--font-size-sm);
          }
          
          .agent-stats {
            .stat-item {
              @include flex-between;
              padding: 4px 0;
              font-size: var(--font-size-sm);
              
              .stat-label {
                color: var(--text-secondary);
              }
              
              .stat-value {
                color: var(--text-primary);
                font-weight: var(--font-weight-medium);
              }
            }
          }
        }
        
        .agent-footer {
          border-top: 1px solid var(--border-extra-light);
          padding-top: 12px;
        }
      }
    }
  }
}

// 响应式适配
@include respond-to(xs) {
  .agent-list {
    .page-header {
      flex-direction: column;
      align-items: flex-start;
      gap: 16px;
      
      .header-right {
        width: 100%;
        justify-content: flex-end;
      }
    }
    
    .agents-grid {
      grid-template-columns: 1fr !important;
    }
  }
}
</style>
