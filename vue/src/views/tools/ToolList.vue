<template>
  <div class="tool-list">
    <!-- 页面头部 -->
    <div class="page-header">
      <div class="header-left">
        <h2 class="page-title">工具列表</h2>
        <p class="page-description">
          {{ serviceFilter ? `查看服务 "${serviceFilter}" 的工具` : '查看和管理所有可用的MCP工具' }}
        </p>
      </div>
      <div class="header-right">
        <el-button
          v-if="serviceFilter"
          @click="clearServiceFilter"
          type="info"
          plain
        >
          查看所有工具
        </el-button>
        <el-button
          :icon="Refresh"
          @click="refreshTools"
          :loading="loading"
        >
          刷新
        </el-button>
      </div>
    </div>
    
    <!-- 统计卡片 -->
    <el-row :gutter="20" class="stats-cards">
      <el-col :xs="24" :sm="8">
        <div class="stat-card">
          <div class="stat-icon tools">
            <el-icon size="24"><Tools /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ systemStore.stats.totalTools }}</div>
            <div class="stat-label">总工具数</div>
          </div>
        </div>
      </el-col>
      <el-col :xs="24" :sm="8">
        <div class="stat-card">
          <div class="stat-icon services">
            <el-icon size="24"><Connection /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ systemStore.stats.totalServices }}</div>
            <div class="stat-label">服务数量</div>
          </div>
        </div>
      </el-col>
      <el-col :xs="24" :sm="8">
        <div class="stat-card">
          <div class="stat-icon categories">
            <el-icon size="24"><Menu /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ Object.keys(toolsByService).length }}</div>
            <div class="stat-label">服务分类</div>
          </div>
        </div>
      </el-col>
    </el-row>
    
    <!-- 搜索和筛选 -->
    <el-card class="filter-card">
      <el-row :gutter="20">
        <el-col :xs="24" :sm="12" :md="8">
          <el-input
            v-model="searchQuery"
            placeholder="搜索工具名称或描述"
            :prefix-icon="Search"
            clearable
            @input="handleSearch"
          />
        </el-col>
        <el-col :xs="24" :sm="12" :md="6">
          <el-select 
            v-model="serviceFilter" 
            placeholder="按服务筛选"
            clearable
            @change="handleFilter"
          >
            <el-option label="全部服务" value="" />
            <el-option 
              v-for="serviceName in serviceNames"
              :key="serviceName"
              :label="serviceName"
              :value="serviceName"
            />
          </el-select>
        </el-col>
        <el-col :xs="24" :sm="12" :md="6">
          <el-select 
            v-model="viewMode" 
            placeholder="视图模式"
          >
            <el-option label="列表视图" value="list" />
            <el-option label="卡片视图" value="card" />
            <el-option label="分组视图" value="group" />
          </el-select>
        </el-col>
        <el-col :xs="24" :sm="12" :md="4">
          <el-button 
            type="primary" 
            :icon="VideoPlay"
            @click="$router.push('/tools/execute')"
          >
            执行工具
          </el-button>
        </el-col>
      </el-row>
    </el-card>
    
    <!-- 工具列表 -->
    <el-card class="tools-card">
      <!-- 列表视图 -->
      <el-table
        v-if="viewMode === 'list'"
        v-loading="loading"
        :data="filteredTools"
        stripe
        style="width: 100%"
      >
        <el-table-column prop="name" label="工具名称" min-width="200">
          <template #default="{ row }">
            <div class="tool-name">
              <el-icon class="tool-icon"><Tools /></el-icon>
              <span>{{ row.name }}</span>
            </div>
          </template>
        </el-table-column>
        
        <el-table-column prop="description" label="描述" min-width="300" show-overflow-tooltip>
          <template #default="{ row }">
            <el-tooltip
              :content="row.description || '暂无描述'"
              placement="top"
              :disabled="!row.description || row.description.length <= 50"
              :show-after="500"
            >
              <span class="tool-description">{{ row.description || '暂无描述' }}</span>
            </el-tooltip>
          </template>
        </el-table-column>

        <el-table-column prop="service_name" label="所属服务" width="200">
          <template #default="{ row }">
            <el-tag size="small" class="service-tag">{{ row.service_name }}</el-tag>
          </template>
        </el-table-column>
        
        <el-table-column label="参数" width="100">
          <template #default="{ row }">
            <el-badge 
              :value="getParameterCount(row)" 
              :max="99"
              class="param-badge"
            >
              <el-icon><Setting /></el-icon>
            </el-badge>
          </template>
        </el-table-column>
        
        <el-table-column label="操作" width="150" fixed="right">
          <template #default="{ row }">
            <div class="action-buttons">
              <el-button 
                size="small" 
                type="primary" 
                @click="executeTool(row)"
              >
                执行
              </el-button>
              <el-button 
                size="small" 
                @click="viewToolDetails(row)"
              >
                详情
              </el-button>
            </div>
          </template>
        </el-table-column>
      </el-table>
      
      <!-- 卡片视图 -->
      <div v-else-if="viewMode === 'card'" class="card-view">
        <el-row :gutter="20">
          <el-col 
            v-for="tool in filteredTools" 
            :key="tool.name"
            :xs="24" :sm="12" :lg="8"
          >
            <div class="tool-card">
              <div class="tool-header">
                <div class="tool-info">
                  <div class="tool-name">{{ tool.name }}</div>
                  <el-tag size="small">{{ tool.service_name }}</el-tag>
                </div>
                <div class="tool-actions">
                  <el-button 
                    type="primary" 
                    size="small"
                    @click="executeTool(tool)"
                  >
                    执行
                  </el-button>
                </div>
              </div>
              
              <div class="tool-body">
                <div class="tool-description">
                  {{ tool.description || '暂无描述' }}
                </div>
                
                <div class="tool-params">
                  <span class="param-label">参数数量:</span>
                  <span class="param-count">{{ getParameterCount(tool) }}</span>
                </div>
              </div>
            </div>
          </el-col>
        </el-row>
      </div>
      
      <!-- 分组视图 -->
      <div v-else-if="viewMode === 'group'" class="group-view">
        <div 
          v-for="(tools, serviceName) in groupedTools" 
          :key="serviceName"
          class="service-group"
        >
          <div class="group-header">
            <h3>{{ serviceName }}</h3>
            <el-badge :value="tools.length" class="group-badge" />
          </div>
          
          <div class="group-tools">
            <div 
              v-for="tool in tools" 
              :key="tool.name"
              class="group-tool-item"
            >
              <div class="tool-info">
                <div class="tool-name">{{ tool.name }}</div>
                <div class="tool-description">{{ tool.description || '暂无描述' }}</div>
              </div>
              <div class="tool-actions">
                <el-button 
                  size="small" 
                  type="primary" 
                  @click="executeTool(tool)"
                >
                  执行
                </el-button>
              </div>
            </div>
          </div>
        </div>
      </div>
      
      <!-- 空状态 -->
      <div v-if="filteredTools.length === 0 && !loading" class="empty-container">
        <el-icon class="empty-icon"><Tools /></el-icon>
        <div class="empty-text">暂无工具</div>
        <div class="empty-description">
          {{ searchQuery || serviceFilter ? '没有找到匹配的工具' : '还没有可用的工具' }}
        </div>
      </div>
    </el-card>
    
    <!-- 工具详情对话框 -->
    <el-dialog
      v-model="detailDialogVisible"
      :title="`工具详情 - ${selectedTool?.name}`"
      width="600px"
    >
      <div v-if="selectedTool" class="tool-details">
        <el-descriptions :column="1" border>
          <el-descriptions-item label="工具名称">
            {{ selectedTool.name }}
          </el-descriptions-item>
          <el-descriptions-item label="所属服务">
            {{ selectedTool.service_name }}
          </el-descriptions-item>
          <el-descriptions-item label="描述">
            {{ selectedTool.description || '暂无描述' }}
          </el-descriptions-item>
          <el-descriptions-item label="参数数量">
            {{ getParameterCount(selectedTool) }}
          </el-descriptions-item>
        </el-descriptions>
        
        <!-- 参数详情 -->
        <div v-if="selectedTool.inputSchema" class="params-section">
          <h4>参数详情</h4>
          <pre>{{ JSON.stringify(selectedTool.inputSchema, null, 2) }}</pre>
        </div>
      </div>
      
      <template #footer>
        <el-button @click="detailDialogVisible = false">关闭</el-button>
        <el-button type="primary" @click="executeTool(selectedTool)">执行工具</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import { ElMessage } from 'element-plus'
import {
  Refresh, Tools, Connection, Menu, Search, VideoPlay, Setting
} from '@element-plus/icons-vue'

const router = useRouter()
const route = useRoute()
const systemStore = useSystemStore()

// 响应式数据
const loading = ref(false)
const searchQuery = ref('')
const serviceFilter = ref('')
const viewMode = ref('list')
const detailDialogVisible = ref(false)
const selectedTool = ref(null)

// 计算属性
const filteredTools = computed(() => {
  let tools = systemStore.tools
  
  // 搜索过滤
  if (searchQuery.value) {
    const query = searchQuery.value.toLowerCase()
    tools = tools.filter(tool => 
      tool.name.toLowerCase().includes(query) ||
      (tool.description && tool.description.toLowerCase().includes(query))
    )
  }
  
  // 服务过滤
  if (serviceFilter.value) {
    tools = tools.filter(tool => tool.service_name === serviceFilter.value)
  }
  
  return tools
})

const serviceNames = computed(() => {
  const names = new Set(systemStore.tools.map(tool => tool.service_name))
  return Array.from(names).sort()
})

const toolsByService = computed(() => systemStore.toolsByService)

const groupedTools = computed(() => {
  const grouped = {}
  filteredTools.value.forEach(tool => {
    const serviceName = tool.service_name || 'unknown'
    if (!grouped[serviceName]) {
      grouped[serviceName] = []
    }
    grouped[serviceName].push(tool)
  })
  return grouped
})

// 方法
const refreshTools = async () => {
  loading.value = true
  try {
    await systemStore.fetchTools()
    ElMessage.success('工具列表刷新成功')
  } catch (error) {
    ElMessage.error('刷新失败')
  } finally {
    loading.value = false
  }
}

const handleSearch = () => {
  // 搜索逻辑已在计算属性中处理
}

const handleFilter = () => {
  // 过滤逻辑已在计算属性中处理
}

const clearServiceFilter = () => {
  serviceFilter.value = ''
  // 清除URL参数
  router.replace({ path: '/tools/list' })
}

const getParameterCount = (tool) => {
  if (!tool.inputSchema || !tool.inputSchema.properties) return 0
  return Object.keys(tool.inputSchema.properties).length
}

const executeTool = (tool) => {
  router.push({
    path: '/tools/execute',
    query: { tool: tool.name }
  })
}

const viewToolDetails = (tool) => {
  selectedTool.value = tool
  detailDialogVisible.value = true
}

// 生命周期
onMounted(async () => {
  // 检查URL参数中是否有服务筛选
  const serviceParam = route.query.service
  if (serviceParam) {
    serviceFilter.value = serviceParam
  }

  await refreshTools()
})
</script>

<style lang="scss" scoped>
.tool-list {
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
  
  .stats-cards {
    margin-bottom: 20px;
    
    .stat-card {
      @include card-shadow;
      padding: 20px;
      display: flex;
      align-items: center;
      gap: 16px;
      
      .stat-icon {
        width: 50px;
        height: 50px;
        border-radius: var(--border-radius-large);
        @include flex-center;
        color: white;
        
        &.tools {
          background: linear-gradient(135deg, #f093fb 0%, #f5576c 100%);
        }
        
        &.services {
          background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        }
        
        &.categories {
          background: linear-gradient(135deg, #4facfe 0%, #00f2fe 100%);
        }
      }
      
      .stat-content {
        .stat-value {
          font-size: 24px;
          font-weight: var(--font-weight-bold);
          color: var(--text-primary);
        }
        
        .stat-label {
          font-size: var(--font-size-sm);
          color: var(--text-secondary);
        }
      }
    }
  }
  
  .filter-card {
    margin-bottom: 20px;
  }
  
  .tools-card {
    .tool-name {
      display: flex;
      align-items: center;
      gap: 8px;
      
      .tool-icon {
        color: var(--primary-color);
      }
    }
    
    .tool-description {
      color: var(--text-regular);
      font-size: var(--font-size-sm);
      line-height: 1.4;
      cursor: pointer;

      // 文本截断样式
      display: -webkit-box;
      -webkit-line-clamp: 2;
      -webkit-box-orient: vertical;
      overflow: hidden;
      text-overflow: ellipsis;
      max-height: 2.8em; // 约2行的高度
    }

    .service-tag {
      max-width: 180px;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .param-badge {
      :deep(.el-badge__content) {
        top: 8px;
        right: 8px;
      }
    }
    
    .action-buttons {
      display: flex;
      gap: 4px;
    }
    
    .card-view {
      .tool-card {
        @include card-shadow;
        padding: 16px;
        margin-bottom: 16px;
        
        .tool-header {
          @include flex-between;
          margin-bottom: 12px;
          
          .tool-info {
            .tool-name {
              font-weight: var(--font-weight-medium);
              margin-bottom: 4px;
            }
          }
        }
        
        .tool-body {
          .tool-description {
            color: var(--text-regular);
            margin-bottom: 8px;
            font-size: var(--font-size-sm);
          }
          
          .tool-params {
            font-size: var(--font-size-sm);
            
            .param-label {
              color: var(--text-secondary);
            }
            
            .param-count {
              color: var(--text-primary);
              font-weight: var(--font-weight-medium);
            }
          }
        }
      }
    }
    
    .group-view {
      .service-group {
        margin-bottom: 24px;
        
        .group-header {
          @include flex-between;
          margin-bottom: 12px;
          padding-bottom: 8px;
          border-bottom: 1px solid var(--border-lighter);
          
          h3 {
            margin: 0;
            color: var(--text-primary);
          }
        }
        
        .group-tools {
          .group-tool-item {
            @include flex-between;
            padding: 12px;
            border: 1px solid var(--border-lighter);
            border-radius: var(--border-radius-base);
            margin-bottom: 8px;
            
            &:hover {
              background: var(--bg-color-page);
            }
            
            .tool-info {
              .tool-name {
                font-weight: var(--font-weight-medium);
                margin-bottom: 4px;
              }
              
              .tool-description {
                font-size: var(--font-size-sm);
                color: var(--text-secondary);
              }
            }
          }
        }
      }
    }
  }
  
  .tool-details {
    .params-section {
      margin-top: 20px;
      
      h4 {
        margin-bottom: 12px;
        color: var(--text-primary);
      }
      
      pre {
        background: var(--bg-color-page);
        padding: 12px;
        border-radius: var(--border-radius-base);
        font-size: var(--font-size-sm);
        max-height: 200px;
        overflow-y: auto;
      }
    }
  }
}

// 响应式适配
@include respond-to(xs) {
  .tool-list {
    .page-header {
      flex-direction: column;
      align-items: flex-start;
      gap: 16px;
      
      .header-right {
        width: 100%;
        justify-content: flex-end;
      }
    }
    
    .action-buttons {
      flex-direction: column;
      
      .el-button {
        width: 100%;
      }
    }
  }
}
</style>
