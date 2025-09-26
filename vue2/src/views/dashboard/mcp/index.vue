<template>
  <div class="mcp-dashboard">
    <!-- 欢迎横幅和图表概览 -->
    <el-row :gutter="20" class="mb-3">
      <el-col :xs="24" :sm="12" :md="12">
        <ArtBasicBanner
          height="160px"
          title="欢迎回来 Super"
          :subtitle="bannerSubtitle"
          :titleColor="'var(--el-text-color-primary)'"
          :subtitleColor="'var(--el-text-color-secondary)'"
          :backgroundColor="systemStatus.running ? 'var(--el-color-primary-light-9)' : 'var(--el-color-danger-light-9)'"
          :buttonConfig="{
            show: true,
            text: '查看详情',
            color: systemStatus.running ? 'var(--el-color-primary)' : 'var(--el-color-danger)',
            textColor: '#fff',
            radius: '6px'
          }"
          @buttonClick="handleBannerClick"
        />
      </el-col>
      <el-col :xs="12" :sm="6" :md="6">
        <div class="card art-custom-card mini-chart-card health-chart-card">
          <div class="health-chart-content">
            <div class="health-text-section">
              <div class="chart-title">服务健康状态</div>
              <div class="health-stats">
                <div class="health-item healthy">
                  <span class="health-dot"></span>
                  <span class="health-count">{{ healthyServiceCount }}</span>
                  <span class="health-label">健康</span>
                </div>
                <div class="health-item unhealthy">
                  <span class="health-dot"></span>
                  <span class="health-count">{{ servicesArray.length - healthyServiceCount }}</span>
                  <span class="health-label">异常</span>
                </div>
              </div>
            </div>
            <div class="health-chart-section">
              <ArtRingChart
                :data="serviceHealthRingData"
                :radius="['40%', '80%']"
                :showLegend="false"
                :colors="['#67c23a', '#f56c6c']"
                height="100px"
              />
            </div>
          </div>
        </div>
      </el-col>
      <el-col :xs="12" :sm="6" :md="6">
        <div class="card art-custom-card mini-chart-card transport-chart-card">
          <div class="transport-chart-content">
            <div class="transport-header">
              <div class="chart-title">传输协议分布</div>
              <div class="transport-stats">
                <span v-for="(stat, index) in serviceTransportStats" :key="stat.name" class="transport-item">
                  <span class="transport-name">{{ stat.name }}</span>
                  <span class="transport-count">{{ stat.value }}</span>
                  <span v-if="index < serviceTransportStats.length - 1" class="transport-separator">|</span>
                </span>
              </div>
            </div>
            <div class="transport-chart-section" style="height: 80px; min-height: 80px;">
              <!-- 专业化的CSS柱状图 -->
              <div class="professional-bar-chart">
                <div 
                  v-for="(item, index) in serviceTransportStats" 
                  :key="item.name"
                  class="chart-bar"
                  :class="`bar-${index}`"
                >
                  <div class="bar-container">
                    <div 
                      class="bar-fill"
                      :style="{
                        height: (item.value / Math.max(...serviceTransportBarData) * 50) + 'px'
                      }"
                    ></div>
                  </div>
                  <div class="bar-info">
                    <div class="bar-label">{{ item.name }}</div>
                    <div class="bar-value">{{ item.value }}</div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </el-col>
    </el-row>

    <!-- 统计卡片 -->
    <el-row :gutter="20" class="mb-3">
      <el-col v-for="(item, index) in statCards" :key="index" :sm="12" :md="6" :lg="6">
        <ArtStatsCard
          :icon="item.icon"
          :title="item.des"
          :value="item.num"
          :description="`${item.changeText}: ${item.change}`"
          :iconSize="24"
          :iconBgRadius="12"
          iconColor="#fff"
          :iconBgColor="getStatCardColor(item.changeClass)"
          :showArrow="true"
          @click="handleCardClick(item.route)"
          class="stat-card-item"
        />
      </el-col>
    </el-row>

    <!-- 第三行：快速操作(4列) + 24小时趋势(8列) -->
    <el-row :gutter="20" class="mb-3">
      <!-- 快速操作卡片 - 4列 -->
      <el-col :xs="24" :sm="12" :md="8">
        <div class="card art-custom-card quick-actions-container">
          <h3 class="box-title">快速操作</h3>
          <div class="actions-grid">
            <div 
              v-for="action in quickActions" 
              :key="action.id"
              class="action-item"
              @click="handleActionClick(action.route, action.action)"
            >
              <div class="action-icon" :style="{ backgroundColor: action.iconBgColor }">
                <i class="iconfont-sys" v-html="action.icon"></i>
              </div>
              <div class="action-content">
                <div class="action-title">{{ action.title }}</div>
                <div class="action-description">{{ action.description }}</div>
              </div>
            </div>
          </div>
        </div>
      </el-col>
      
      <!-- 24小时趋势图表 - 8列 -->
      <el-col :xs="24" :sm="12" :md="16">
        <div class="card art-custom-card chart-card-wide">
          <div class="chart-header">
            <h3 class="box-title">24小时调用趋势</h3>
          </div>
          <div class="chart-container">
            <ArtLineChart
              :data="hourlyChartData"
              :xAxisData="hourlyLabels"
              :showAreaColor="true"
              :isEmpty="false"
              height="280px"
              :yAxisConfig="{ min: 0, max: null }"
            />
          </div>
        </div>
      </el-col>
    </el-row>

    <!-- 第四行：30天趋势(8列) + 工具调用记录(4列) -->
    <el-row :gutter="20">
      <!-- 30天长趋势图表 - 8列 -->
      <el-col :xs="24" :sm="12" :md="16">
        <div class="card art-custom-card long-chart-card">
          <div class="chart-header">
            <h3 class="box-title">30天使用趋势</h3>
          </div>
          <div class="long-chart-container">
            <ArtLineChart
              :data="monthlyChartData"
              :xAxisData="monthlyLabels"
              :showAreaColor="true"
              height="280px"
              :yAxisConfig="{ min: 0, max: null }"
            />
          </div>
        </div>
      </el-col>
      
      <!-- 工具调用记录 - 4列 -->
      <el-col :xs="24" :sm="12" :md="8">
        <div class="card art-custom-card tool-records-compact">
          <h3 class="box-title">最近调用</h3>
          <ArtDataListCard
            :list="formattedToolRecords"
            title=""
            :maxCount="6"
            :showMoreButton="false"
          />
        </div>
      </el-col>
    </el-row>


  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, computed, ref, watch, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { useDashboardData } from '../../../composables/useDashboardData'
// ArtStatsCard 已全局注册，无需导入

const router = useRouter()

// 使用仪表盘数据 composable
const {
  loading,
  error,
  services,
  toolsData,
  toolRecords,
  systemResources,
  agentsSummary,
  healthSummary,
  statCards,
  serviceHealthRingData,
  healthyServiceCount,
  serviceTransportStats,
  serviceTransportBarData,
  serviceTransportLabels,
  weeklyBarData,
  weeklyLabels,
  hourlyChartData,
  hourlyLabels,
  monthlyChartData,
  monthlyLabels,
  systemStatus,
  systemInfo,
  quickActions,
  refreshData
} = useDashboardData()

// 兼容性处理 - 将工具数据转换为旧格式
const tools = computed(() => {
  const data = toolsData.value?.data || toolsData.value || {}
  return data.tools || data || []
})

// 兼容性处理 - 将 Agent 数据转换为旧格式
const agents = computed(() => {
  return agentsSummary.value?.agents?.map((agent: any) => agent.agent_id) || []
})

// 兼容性处理 - 将服务数据转换为旧格式
const servicesArray = computed(() => {
  const data = services.value?.data || services.value || {}
  return data.services || []
})


// 统计卡片颜色映射
const getStatCardColor = (changeClass: string) => {
  const colorMap = {
    'text-success': 'var(--el-color-success)',
    'text-warning': 'var(--el-color-warning)', 
    'text-info': 'var(--el-color-info)',
    'text-danger': 'var(--el-color-danger)'
  }
  return colorMap[changeClass as keyof typeof colorMap] || 'var(--el-color-primary)'
}

// 图标配置
const iconConfig = {
  success: '&#xe63f;', // 成功图标
  error: '&#xe63e;'     // 失败图标
}

// 格式化工具记录为ArtDataListCard需要的格式
const formattedToolRecords = computed(() => {
  const executions = toolRecords.value?.data?.executions || []
  return executions.map((record: any) => ({
    title: record.tool_name,
    status: record.service_name,
    time: formatTimeAgo(record),
    class: record.error ? 'error-item' : 'success-item',
    icon: record.error ? iconConfig.error : iconConfig.success
  }))
})

// 横幅动态字幕
const bannerSubtitle = computed(() => {
  const serviceCount = servicesArray.value.length
  const toolCount = tools.value.length
  const executionCount = Array.isArray(toolRecords.value) ? toolRecords.value.length : 0
  const successRate = Array.isArray(toolRecords.value) && toolRecords.value.length > 0
    ? Math.round((toolRecords.value.filter((r: any) => !r.is_error).length / toolRecords.value.length) * 100)
    : 0

  if (systemStatus.value.running) {
    return `MCP 系统运行正常，当前有 ${serviceCount} 个服务、${toolCount} 个工具，今日调用 ${executionCount} 次，成功率 ${successRate}%。`
  } else {
    return `MCP 系统状态异常，请检查服务配置。当前有 ${serviceCount} 个服务配置。`
  }
})

// 总执行次数
const totalExecutions = computed(() => {
  return Array.isArray(toolRecords.value) ? toolRecords.value.length : 0
})

// 执行结果环形图数据
const executionRingData = computed(() => {
  const total = Array.isArray(toolRecords.value) ? toolRecords.value.length : 0
  if (total === 0) {
    return [{ value: 1, name: '暂无数据' }]
  }

  const successful = toolRecords.value.filter((r: any) => !r.is_error).length
  const failed = total - successful

  return [
    { value: successful, name: '成功' },
    { value: failed, name: '失败' }
  ]
})

// 服务柱状图数据（模拟一周的服务活动）
const serviceBarData = computed(() => {
  // 基于当前服务数量生成模拟的一周数据
  const baseCount = servicesArray.value.length
  return [
    Math.max(1, baseCount - 2),
    Math.max(1, baseCount - 1),
    baseCount,
    Math.max(1, baseCount - 1),
    baseCount,
    Math.max(1, baseCount + 1),
    baseCount
  ]
})

// 注意：quickActions 和 statCards 现在从 useDashboardData composable 中获取

// 方法
const handleCardClick = (route: string) => {
  if (route) {
    router.push(route)
  }
}

// 时间格式：xx分钟前 / xx小时前 / xx天前
const formatTimeAgo = (record: any) => {
  const now = Date.now()
  const t = record.execution_time ? new Date(record.execution_time).getTime() : (record.timestamp ? record.timestamp * 1000 : now)
  const diff = Math.max(0, now - t)
  const minute = 60 * 1000
  const hour = 60 * minute
  const day = 24 * hour
  if (diff < minute) return '刚刚'
  if (diff < hour) return Math.floor(diff / minute) + '分钟前'
  if (diff < day) return Math.floor(diff / hour) + '小时前'
  return Math.floor(diff / day) + '天前'
}

const handleActionClick = (route: string | null, action: string | null) => {
  if (route) {
    router.push(route)
  } else if (action === 'refresh') {
    refreshData()
  }
}

const handleBannerClick = () => {
  // 点击横幅按钮时跳转到系统资源页面或刷新数据
  if (systemStatus.value.running) {
    router.push('/services')
  } else {
    refreshData()
  }
}


// refreshData 方法现在从 useDashboardData composable 中获取

onMounted(() => {
  // 初始加载数据
  refreshData()
  
  // 数据加载完成
})

// 移除调试代码，保持代码整洁
</script>

<style lang="scss" scoped>
.mcp-dashboard {
  padding: 20px;
  min-height: calc(100vh - 120px); // 确保页面高度适合视窗
  box-sizing: border-box;

  .mb-3 {
    margin-bottom: 20px;
  }

  // 确保所有行都有合适的间距
  .el-row {
    &:not(:last-child) {
      margin-bottom: 20px;
    }
  }

  .stat-card-item {
    cursor: pointer;
    transition: all 0.3s ease;

    &:hover {
      transform: translateY(-2px);
      box-shadow: 0 8px 25px rgba(0, 0, 0, 0.15);
    }

    &:active {
      transform: translateY(0);
    }
  }

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

  .quick-action-card {
    cursor: pointer;
    transition: all 0.3s ease;

    &:hover {
      transform: translateY(-2px);
      box-shadow: 0 8px 25px rgba(0, 0, 0, 0.15);
    }

    &:active {
      transform: translateY(0);
    }
  }

  // 快速操作卡片容器 - 4列宽度
  .quick-actions-container {
    height: 360px;
    
    .box-title {
      margin-bottom: 16px;
      font-size: 16px;
      color: var(--el-text-color-primary);
      font-weight: 600;
    }
    
    .actions-grid {
      display: grid;
      grid-template-columns: 1fr 1fr;
      grid-template-rows: 1fr 1fr;
      gap: 12px;
      height: calc(100% - 50px);
    }
    
    .action-item {
      display: flex;
      align-items: center;
      padding: 18px 16px;
      background: var(--el-bg-color);
      border: 1px solid var(--el-border-color-lighter);
      border-radius: 12px;
      cursor: pointer;
      transition: all 0.3s ease;
      min-height: 90px;
      
      &:hover {
        background: var(--el-fill-color-light);
        border-color: var(--el-color-primary);
        transform: translateY(-2px);
        box-shadow: 0 4px 12px rgba(64, 158, 255, 0.12);
      }
      
      .action-icon {
        width: 40px;
        height: 40px;
        border-radius: 10px;
        display: flex;
        align-items: center;
        justify-content: center;
        margin-right: 14px;
        flex-shrink: 0;
        
        i {
          color: #fff;
          font-size: 18px;
          font-weight: normal;
          line-height: 1;
          font-family: 'iconfont-sys' !important;
          font-style: normal;
          display: inline-block;
          -webkit-font-smoothing: antialiased;
          -moz-osx-font-smoothing: grayscale;
        }
      }
      
      .action-content {
        flex: 1;
        min-width: 0; // 允许文本截断
        display: flex;
        flex-direction: column;
        justify-content: center;
        
        .action-title {
          font-size: 15px;
          font-weight: 600;
          color: var(--el-text-color-primary);
          margin-bottom: 4px;
          line-height: 1.2;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }
        
        .action-description {
          font-size: 12px;
          color: var(--el-text-color-secondary);
          line-height: 1.3;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }
      }
    }
  }

  // 24小时图表卡片 - 8列宽度
  .chart-card-wide {
    height: 360px;
    
    .chart-header {
      margin-bottom: 16px;
      
      .box-title {
        margin: 0;
        font-size: 18px;
        color: var(--el-text-color-primary);
        font-weight: 600;
      }
    }
    
    .chart-container {
      height: 280px;
    }
  }

  // 30天长趋势图表 - 8列宽度
  .long-chart-card {
    height: 360px;
    
    .chart-header {
      margin-bottom: 16px;
      
      .box-title {
        margin: 0;
        font-size: 18px;
        color: var(--el-text-color-primary);
        font-weight: 600;
      }
    }
    
    .long-chart-container {
      height: 280px;
    }
  }

  // 工具记录卡片 - 4列宽度
  .tool-records-compact {
    height: 360px;
    overflow: hidden; // 防止内容溢出
    
    .box-title {
      margin-bottom: 12px;
      font-size: 16px;
    }
    
    // 完全重写ArtDataListCard样式，去除边框和内边距
    :deep(.basic-list-card) {
      padding: 0 !important;
      background: transparent !important;
      border-radius: 0 !important;
      
      .art-card {
        background: transparent !important;
        border: none !important;
        border-radius: 0 !important;
        box-shadow: none !important;
        padding: 0 !important;
        margin: 0 !important;
      }
      
      .card-header {
        display: none !important; // 隐藏卡片头部
      }
      
      // 滚动区域
      .el-scrollbar {
        height: 300px !important; // 固定滚动区域高度
        
        .el-scrollbar__wrap {
          padding: 0 !important;
        }
        
        .el-scrollbar__view {
          padding: 0 !important;
        }
      }
      
      // 列表项样式
      .list-item {
        padding: 12px 0 !important;
        margin: 0 !important;
        border-bottom: 1px solid var(--el-border-color-lighter) !important;
        background: transparent !important;
        transition: background-color 0.2s ease;
        
        &:hover {
          background-color: var(--el-fill-color-light) !important;
          border-radius: 4px;
          margin: 0 -8px !important;
          padding: 12px 8px !important;
        }
        
        &:last-child {
          border-bottom: none !important;
        }
        
        // 图标区域
        .item-icon {
          width: 24px !important;
          height: 24px !important;
          margin-right: 8px !important;
          border-radius: 4px !important;
          
          i {
            font-size: 12px !important;
          }
        }
        
        // 内容区域
        .item-content {
          flex: 1 !important;
          
          .item-title {
            font-size: 13px !important;
            font-weight: 500 !important;
            color: var(--el-text-color-primary) !important;
            margin-bottom: 2px !important;
            line-height: 1.3 !important;
          }
          
          .item-status {
            font-size: 11px !important;
            color: var(--el-text-color-secondary) !important;
            line-height: 1.2 !important;
          }
        }
        
        // 时间区域
        .item-time {
          font-size: 11px !important;
          color: var(--el-text-color-secondary) !important;
          min-width: 50px !important;
          text-align: right !important;
          flex-shrink: 0 !important;
        }
      }
    }
  }

  .mini-chart-card {
    height: 160px; // 与横幅高度一致
    display: flex;
    flex-direction: column;
    
    .chart-header {
      flex-shrink: 0; // 固定头部大小
      padding: 12px 0 8px 0;
      text-align: center;
      
      .chart-value {
        font-size: 24px;
        font-weight: 600;
        color: var(--el-text-color-primary);
        line-height: 1.2;
      }
      
      .chart-label {
        font-size: 12px;
        color: var(--el-text-color-secondary);
        margin-top: 4px;
      }
    }
    
    .chart-container-mini {
      flex: 1; // 占剩余空间
      display: flex;
      align-items: center;
      justify-content: center;
      min-height: 0; // 允许收缩
      padding: 8px; // 给图表一些内边距
    }
  }

  // 健康服务占比卡片 - 左右布局
  .health-chart-card {
    // 移除可能干扰的样式重置
    /* .chart-header,
    .chart-container-mini {
      display: none !important;
    } */
    
    .health-chart-content {
      display: flex;
      align-items: center;
      height: 100%;
      padding: 16px 12px;
      
      .health-text-section {
        flex: 1;
        
        .chart-title {
          font-size: 13px;
          font-weight: 600;
          color: var(--el-text-color-primary);
          margin-bottom: 12px;
        }
        
        .health-stats {
          .health-item {
            display: flex;
            align-items: center;
            margin-bottom: 8px;
            
            &:last-child {
              margin-bottom: 0;
            }
            
            .health-dot {
              width: 8px;
              height: 8px;
              border-radius: 50%;
              margin-right: 6px;
            }
            
            .health-count {
              font-size: 16px;
              font-weight: 600;
              margin-right: 4px;
            }
            
            .health-label {
              font-size: 12px;
              color: var(--el-text-color-secondary);
            }
            
            &.healthy {
              .health-dot {
                background-color: var(--el-color-success);
              }
              .health-count {
                color: var(--el-color-success);
              }
            }
            
            &.unhealthy {
              .health-dot {
                background-color: var(--el-color-danger);
              }
              .health-count {
                color: var(--el-color-danger);
              }
            }
          }
        }
      }
      
      .health-chart-section {
        width: 100px;
        height: 100px;
        flex-shrink: 0;
      }
    }
  }

  // 传输协议分布卡片 - 上下布局
  .transport-chart-card {
    // 暂时移除可能有问题的样式重置
    /* .chart-header,
    .chart-container-mini {
      display: none !important;
    } */
    
    .transport-chart-content {
      height: 100%;
      min-height: 128px;
      padding: 16px 12px;
      display: flex;
      flex-direction: column;
      
      .transport-header {
        flex-shrink: 0;
        margin-bottom: 16px;
        
        .chart-title {
          font-size: 13px;
          font-weight: 600;
          color: var(--el-text-color-primary);
          margin-bottom: 8px;
        }
        
        .transport-stats {
          display: flex;
          align-items: center;
          
          .transport-item {
            display: flex;
            align-items: center;
            font-size: 12px;
            
            .transport-name {
              color: var(--el-text-color-primary);
              margin-right: 4px;
            }
            
            .transport-count {
              color: var(--el-color-primary);
              font-weight: 600;
              margin-right: 6px;
            }
            
            .transport-separator {
              color: var(--el-text-color-placeholder);
              margin: 0 6px;
            }
          }
        }
      }
      
        .transport-chart-section {
          flex: 1;
          display: flex;
          align-items: center;
          min-height: 80px;
          height: 80px;
          position: relative;
          
          .professional-bar-chart {
            display: flex;
            align-items: flex-end;
            justify-content: space-evenly;
            width: 100%;
            height: 70px;
            padding: 0 10px;
            
            .chart-bar {
              display: flex;
              flex-direction: column;
              align-items: center;
              justify-content: flex-end;
              transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
              
              .bar-container {
                position: relative;
                width: 32px;
                height: 50px;
                display: flex;
                align-items: flex-end;
                justify-content: center;
                
                .bar-fill {
                  width: 100%;
                  border-radius: 6px 6px 0 0;
                  transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1);
                  position: relative;
                  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
                  
                  &::before {
                    content: '';
                    position: absolute;
                    top: 0;
                    left: 0;
                    right: 0;
                    height: 3px;
                    border-radius: 6px 6px 0 0;
                    background: linear-gradient(90deg, rgba(255,255,255,0.3) 0%, rgba(255,255,255,0.1) 100%);
                  }
                }
              }
              
              .bar-info {
                margin-top: 8px;
                text-align: center;
                
                .bar-label {
                  font-size: 11px;
                  color: var(--el-text-color-regular);
                  font-weight: 500;
                  margin-bottom: 2px;
                }
                
                .bar-value {
                  font-size: 13px;
                  color: var(--el-text-color-primary);
                  font-weight: 600;
                }
              }
              
              &:hover {
                transform: translateY(-3px);
                
                .bar-container .bar-fill {
                  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.2);
                  filter: brightness(1.1);
                }
              }
            }
            
            // 第一个柱子 - Stdio (蓝色)
            .bar-0 .bar-container .bar-fill {
              background: linear-gradient(180deg, #66b3ff 0%, #409eff 100%);
            }
            
            // 第二个柱子 - HTTP (绿色)  
            .bar-1 .bar-container .bar-fill {
              background: linear-gradient(180deg, #85ce61 0%, #67c23a 100%);
            }
          }
        }
    }
  }

  .chart-card {
    .chart-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 16px;

      .box-title {
        margin: 0;
      }

      .time-range-selector {
        :deep(.el-radio-group) {
          .el-radio-button {
            &:first-child .el-radio-button__inner {
              border-left: 1px solid var(--el-border-color);
            }
          }
        }
      }
    }

    .chart-container {
      height: 300px;
      padding: 10px 0;
    }
  }


  // 响应式设计
  @media (max-width: 768px) {
    padding: 12px;
    min-height: calc(100vh - 100px); // 移动端调整

    .el-row {
      margin-bottom: 12px !important; // 移动端减少间距
    }

    .quick-actions-container {
      height: auto;
      margin-bottom: 12px;
      
      .actions-grid {
        grid-template-rows: auto auto;
        gap: 8px;
        height: auto;
      }
      
      .action-item {
        padding: 14px 12px;
        min-height: 75px;
        border-radius: 10px;
        
        .action-icon {
          width: 32px;
          height: 32px;
          margin-right: 10px;
          border-radius: 8px;
          
          i {
            font-size: 15px !important;
            font-family: 'iconfont-sys' !important;
            font-style: normal;
            font-weight: normal;
            -webkit-font-smoothing: antialiased;
            -moz-osx-font-smoothing: grayscale;
          }
        }
        
        .action-content {
          .action-title {
            font-size: 13px !important;
            margin-bottom: 3px;
          }
          
          .action-description {
            font-size: 11px !important;
          }
        }
      }
    }
    
    .chart-card-wide,
    .long-chart-card {
      height: 280px;
      margin-bottom: 12px;
      
      .chart-container,
      .long-chart-container {
        height: 200px;
      }
    }
    
    .tool-records-compact {
      height: 280px;
      
      :deep(.el-scrollbar) {
        height: 220px !important;
      }
    }

    .mini-chart-card {
      margin-bottom: 12px;
    }

    .health-chart-card {
      .health-chart-content {
        padding: 12px 8px;
        
        .health-text-section {
          .chart-title {
            font-size: 12px;
            margin-bottom: 8px;
          }
          
          .health-stats .health-item {
            margin-bottom: 6px;
            
            .health-count {
              font-size: 14px;
            }
            
            .health-label {
              font-size: 11px;
            }
          }
        }
        
        .health-chart-section {
          width: 80px;
          height: 80px;
        }
      }
    }

    .transport-chart-card {
      .transport-chart-content {
        padding: 12px 8px;
        
        .transport-header {
          margin-bottom: 12px;
          
          .chart-title {
            font-size: 12px;
            margin-bottom: 6px;
          }
          
          .transport-stats .transport-item {
            font-size: 11px;
          }
        }
      }
    }
  }

  // 超小屏幕优化
  @media (max-width: 480px) {
    padding: 8px;

    .quick-actions-container,
    .chart-card-wide,
    .long-chart-card,
    .tool-records-compact {
      height: auto;
      min-height: 200px;
    }
  }
}
</style>
