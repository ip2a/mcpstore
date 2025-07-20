<template>
  <div class="agent-list">
    <!-- 页面头部 -->
    <div class="page-header">
      <div class="header-left">
        <h2 class="page-title">Agent列表</h2>
        <p class="page-description">管理所有Agent实例</p>
      </div>
      <div class="header-right">
        <el-button 
          type="primary" 
          :icon="Plus" 
          @click="$router.push('/agents/create')"
        >
          创建Agent
        </el-button>
        <el-button 
          :icon="Refresh" 
          @click="refreshAgents"
          :loading="loading"
        >
          刷新
        </el-button>
      </div>
    </div>
    
    <!-- Agent列表 -->
    <el-card class="agents-card">
      <div v-if="agents.length === 0 && !loading" class="empty-container">
        <el-icon class="empty-icon"><User /></el-icon>
        <div class="empty-text">暂无Agent</div>
        <div class="empty-description">还没有创建任何Agent实例</div>
        <el-button 
          type="primary" 
          @click="$router.push('/agents/create')"
        >
          创建第一个Agent
        </el-button>
      </div>
      
      <div v-else class="agents-grid">
        <div 
          v-for="agent in agents" 
          :key="agent.id"
          class="agent-card"
        >
          <div class="agent-header">
            <div class="agent-info">
              <div class="agent-name">{{ agent.name }}</div>
              <div class="agent-id">ID: {{ agent.id }}</div>
            </div>
            <el-tag 
              :type="agent.status === 'active' ? 'success' : 'info'"
              size="small"
            >
              {{ agent.status === 'active' ? '活跃' : '非活跃' }}
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
                <span class="stat-label">创建时间:</span>
                <span class="stat-value">{{ formatTime(agent.created_at) }}</span>
              </div>
            </div>
          </div>
          
          <div class="agent-footer">
            <el-button-group>
              <el-button 
                size="small" 
                @click="viewAgent(agent)"
              >
                查看
              </el-button>
              <el-button 
                size="small" 
                type="primary"
                @click="manageAgent(agent)"
              >
                管理
              </el-button>
              <el-button 
                size="small" 
                type="danger"
                @click="deleteAgent(agent)"
              >
                删除
              </el-button>
            </el-button-group>
          </div>
        </div>
      </div>
    </el-card>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import dayjs from 'dayjs'
import { Plus, Refresh, User } from '@element-plus/icons-vue'

const router = useRouter()

// 响应式数据
const loading = ref(false)
const agents = ref([
  {
    id: 'agent_001',
    name: '测试Agent',
    description: '这是一个测试Agent实例',
    status: 'active',
    services: 3,
    tools: 8,
    created_at: new Date().toISOString()
  }
])

// 方法
const refreshAgents = async () => {
  loading.value = true
  try {
    // 这里应该调用获取Agent列表的API
    await new Promise(resolve => setTimeout(resolve, 1000))
    ElMessage.success('Agent列表刷新成功')
  } catch (error) {
    ElMessage.error('刷新失败')
  } finally {
    loading.value = false
  }
}

const viewAgent = (agent) => {
  ElMessage.info(`查看Agent: ${agent.name}`)
}

const manageAgent = (agent) => {
  router.push(`/agents/${agent.id}/manage`)
}

const deleteAgent = async (agent) => {
  try {
    await ElMessageBox.confirm(
      `确定要删除Agent "${agent.name}" 吗？`,
      '删除确认',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )
    
    // 这里应该调用删除Agent的API
    agents.value = agents.value.filter(a => a.id !== agent.id)
    ElMessage.success(`Agent ${agent.name} 删除成功`)
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error(`Agent ${agent.name} 删除失败`)
    }
  }
}

const formatTime = (time) => {
  return dayjs(time).format('YYYY-MM-DD HH:mm')
}

// 生命周期
onMounted(async () => {
  await refreshAgents()
})
</script>

<style lang="scss" scoped>
.agent-list {
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
