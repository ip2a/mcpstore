<template>
  <div class="agents-page art-full-height">
    <el-row :gutter="20" class="mb-3">
      <el-col :xs="24" :sm="12" :md="12">
        <ArtBasicBanner
          title="Agent 概览"
          :subtitle="headerSubtitle"
          :titleColor="'var(--el-text-color-primary)'"
          :subtitleColor="'var(--el-text-color-secondary)'"
          :backgroundColor="'var(--el-color-warning-light-9)'"
          :buttonConfig="{ 
            show: true, 
            text: '刷新数据', 
            color: 'var(--el-color-warning)', 
            textColor: '#fff', 
            radius: '6px' 
          }"
          @buttonClick="loadData"
        />
      </el-col>
      <el-col :xs="12" :sm="6" :md="6">
        <div class="card art-custom-card mini-chart-card">
          <div class="chart-header">
            <div class="chart-value">{{ activeRate }}%</div>
            <div class="chart-label">活跃率</div>
          </div>
          <div class="chart-container-mini">
            <ArtRingChart
              :data="activeRateChartData"
              :radius="['50%', '80%']"
              :showLegend="false"
              height="120px"
            />
          </div>
        </div>
      </el-col>
      <el-col :xs="12" :sm="6" :md="6">
        <div class="card art-custom-card mini-chart-card">
          <div class="chart-header">
            <div class="chart-value">{{ summary.total_services }}</div>
            <div class="chart-label">服务总数</div>
          </div>
          <div class="chart-container-mini">
            <ArtBarChart
              :data="serviceDistributionData"
              :xAxisData="serviceDistributionLabels"
              :showLegend="false"
              height="120px"
              barWidth="60%"
            />
          </div>
        </div>
      </el-col>
    </el-row>

    <el-card class="art-table-card table-card-enhanced" shadow="hover">
      <ArtTableHeader :loading="loading" @refresh="loadData" />
      <el-row :gutter="16">
        <el-col v-for="agent in agents" :key="agent.agent_id" :xs="24" :md="12">
          <div class="card art-custom-card agent-card">
            <div class="agent-header">
              <div class="left">
                <div class="agent-id">{{ agent.agent_id }}</div>
                <div class="agent-meta">
                  服务: {{ agent.service_count }} | 工具: {{ agent.tool_count }} | 健康: {{ agent.healthy_services }}/{{ agent.service_count }}
                </div>
              </div>
              <div class="right">
                <el-tag type="success" size="small" v-if="agent.healthy_services === agent.service_count">健康</el-tag>
                <el-tag type="warning" size="small" v-else>部分异常</el-tag>
              </div>
            </div>
            <el-divider />
            <div class="services">
              <el-space direction="vertical" fill>
                <el-card
                  v-for="svc in agent.services || []"
                  :key="svc.client_id || svc.service_name"
                  class="mini-svc"
                  shadow="never"
                >
                  <template #header>
                    <div class="svc-header">
                      <span class="svc-name">{{ svc.service_name }}</span>
                      <el-tag size="small" :type="getStatusType(svc.status)">{{ svc.status }}</el-tag>
                    </div>
                  </template>
                  <div class="svc-body">
                    <div class="meta">类型：{{ svc.service_type }}，工具：{{ svc.tool_count }}</div>
                    <div v-if="svc.client_id" class="meta">Client ID：{{ svc.client_id }}</div>
                  </div>
                </el-card>
              </el-space>
            </div>
          </div>
        </el-col>
      </el-row>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref, computed } from 'vue'
import { ElMessage } from 'element-plus'
import { dashboardApi } from '@/mcp/api/dashboard'

type AgentItem = {
  agent_id: string
  service_count: number
  tool_count: number
  healthy_services: number
  unhealthy_services: number
  total_tool_executions: number
  last_activity: string | null
  services: Array<{
    service_name: string
    service_type: string
    status: string
    tool_count: number
    client_id?: string
    response_time?: number | null
  }>
}

const loading = ref(false)
const summary = ref({ total_agents: 0, active_agents: 0, total_services: 0, total_tools: 0 })
const agents = ref<AgentItem[]>([])

// 计算属性
const headerSubtitle = computed(() => {
  return `共 ${summary.value.total_agents} 个Agent，活跃 ${summary.value.active_agents} 个，管理 ${summary.value.total_services} 个服务和 ${summary.value.total_tools} 个工具`
})

const activeRate = computed(() => {
  if (summary.value.total_agents === 0) return 100
  return Math.round((summary.value.active_agents / summary.value.total_agents) * 100)
})

const activeRateChartData = computed(() => [
  { value: activeRate.value, name: '活跃' },
  { value: 100 - activeRate.value, name: '非活跃' }
])

const serviceDistributionData = computed(() => {
  // 按Agent分组显示服务分布
  const agentServices = agents.value.map(agent => agent.service_count)
  return agentServices.length > 0 ? agentServices : [1, 2, 3] // 默认数据以防空数组
})

const serviceDistributionLabels = computed(() => {
  const labels = agents.value.map(agent => `Agent${agent.agent_id.slice(-4)}`)
  return labels.length > 0 ? labels : ['A1', 'A2', 'A3'] // 默认标签
})

const getStatusType = (status: string) => (status === 'healthy' ? 'success' : status === 'warning' ? 'warning' : 'info')

const loadData = async () => {
  try {
    loading.value = true
    const res = await dashboardApi.getAgentsSummary()
    const data = res?.data || {}
    summary.value = {
      total_agents: data.total_agents || 0,
      active_agents: data.active_agents || 0,
      total_services: data.total_services || 0,
      total_tools: data.total_tools || 0
    }
    agents.value = Array.isArray(data.agents) ? data.agents : []
  } catch (e) {
    ElMessage.error('获取 Agent 列表失败')
  } finally {
    loading.value = false
  }
}

onMounted(loadData)
</script>

<style scoped lang="scss">
.agents-page {
  padding: 20px;

  .mb-3 {
    margin-bottom: 20px;
  }

  // 卡片基础样式
  .card,
  .art-custom-card {
    background: var(--el-bg-color);
    border: 1px solid var(--el-border-color-light);
    border-radius: 8px;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
    padding: 20px;
    margin-bottom: 20px;
    transition: all 0.3s ease;

    &:hover {
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    }

    .box-title {
      font-size: 18px;
      font-weight: 500;
      color: var(--el-text-color-primary);
      margin-bottom: 16px;
    }

    .subtitle {
      color: var(--el-text-color-secondary);
      font-size: 14px;
    }

    .text-success { color: var(--el-color-success); }
    .text-warning { color: var(--el-color-warning); }
    .text-info { color: var(--el-color-info); }
    .text-danger { color: var(--el-color-danger); }
  }

  // 小图表卡片样式
  .mini-chart-card {
    .chart-header {
      margin-bottom: 12px;

      .chart-value {
        font-size: 24px;
        font-weight: 600;
        color: var(--el-text-color-primary);
        margin-bottom: 4px;
      }

      .chart-label {
        font-size: 14px;
        color: var(--el-text-color-secondary);
      }
    }

    .chart-container-mini {
      height: 120px;
    }
  }

  // 增强表格卡片样式
  .table-card-enhanced {
    background: var(--el-bg-color);
    border: 1px solid var(--el-border-color-light);
    border-radius: 12px;
    box-shadow: 0 4px 6px rgba(0, 0, 0, 0.07);
    overflow: hidden;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);

    &:hover {
      box-shadow: 0 8px 20px rgba(0, 0, 0, 0.12);
      transform: translateY(-1px);
      border-color: var(--el-color-warning-light-7);
    }
  }

  // Agent卡片样式
  .agent-card {
    margin-bottom: 16px;
    border: 1px solid var(--el-border-color-light);
    border-radius: 8px;
    transition: all 0.3s ease;

    &:hover {
      border-color: var(--el-color-warning-light-5);
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    }

    .agent-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 16px;

      .left {
        flex: 1;

        .agent-id {
          font-weight: 600;
          font-size: 16px;
          color: var(--el-text-color-primary);
          margin-bottom: 4px;
        }

        .agent-meta {
          color: var(--el-text-color-secondary);
          font-size: 12px;
          line-height: 1.4;
        }
      }

      .right {
        flex-shrink: 0;
      }
    }

    .services {
      .mini-svc {
        margin-bottom: 8px;
        border: 1px solid var(--el-border-color-lighter);
        border-radius: 6px;
        transition: all 0.2s ease;

        &:hover {
          border-color: var(--el-border-color);
        }

        :deep(.el-card__header) {
          padding: 12px 16px;
          border-bottom: 1px solid var(--el-border-color-lighter);
        }

        :deep(.el-card__body) {
          padding: 12px 16px;
        }

        .svc-header {
          display: flex;
          justify-content: space-between;
          align-items: center;

          .svc-name {
            font-weight: 600;
            color: var(--el-text-color-primary);
          }
        }

        .svc-body {
          .meta {
            font-size: 13px;
            color: var(--el-text-color-secondary);
            line-height: 1.5;
            margin-bottom: 4px;

            &:last-child {
              margin-bottom: 0;
            }
          }
        }
      }
    }
  }

  // 分隔线样式
  :deep(.el-divider) {
    margin: 16px 0;
    border-color: var(--el-border-color-lighter);
  }

  // 标签样式优化
  :deep(.el-tag) {
    font-weight: 500;
    border-radius: 4px;
  }

  // 响应式设计
  @media (max-width: 768px) {
    padding: 12px;

    .card,
    .art-custom-card {
      padding: 16px;
      margin-bottom: 16px;
    }

    .mini-chart-card {
      .chart-header {
        margin-bottom: 8px;

        .chart-value {
          font-size: 20px;
        }

        .chart-label {
          font-size: 12px;
        }
      }

      .chart-container-mini {
        height: 100px;
      }
    }

    .agent-card {
      .agent-header {
        flex-direction: column;
        align-items: flex-start;
        gap: 8px;

        .left .agent-id {
          font-size: 15px;
        }

        .left .agent-meta {
          font-size: 11px;
        }
      }

      .services .mini-svc {
        :deep(.el-card__header) {
          padding: 10px 12px;
        }

        :deep(.el-card__body) {
          padding: 10px 12px;
        }

        .svc-body .meta {
          font-size: 12px;
        }
      }
    }
  }
}
</style>

