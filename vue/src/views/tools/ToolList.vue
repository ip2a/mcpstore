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
      <el-row :gutter="16">
        <el-col :xs="24" :sm="12" :md="8">
          <el-input
            v-model="searchQuery"
            placeholder="Search by name or description"
            :prefix-icon="Search"
            clearable
            @input="handleSearch"
          />
        </el-col>
        <el-col :xs="24" :sm="12" :md="6">
          <el-select
            v-model="serviceFilter"
            placeholder="Filter by service"
            clearable
            @change="handleFilter"
          >
            <el-option label="All Services" value="" />
            <el-option
              v-for="serviceName in serviceNames"
              :key="serviceName"
              :label="serviceName"
              :value="serviceName"
            />
          </el-select>
        </el-col>
        <el-col :xs="24" :sm="12" :md="4">
          <el-button
            type="primary"
            :icon="Refresh"
            @click="refreshTools"
            :loading="loading"
            style="width: 100%"
          >
            Refresh
          </el-button>
        </el-col>
      </el-row>
    </el-card>

    <!-- 工具列表 -->
    <el-card class="tools-card">
      <el-table
        v-loading="loading"
        :data="filteredTools"
        stripe
        style="width: 100%"
      >
        <el-table-column type="index" label="#" width="60" />

        <el-table-column prop="name" label="Name" min-width="200" />

        <el-table-column prop="service" label="Service" width="180" />

        <el-table-column prop="description" label="Description" min-width="300" show-overflow-tooltip>
          <template #default="{ row }">
            <span>{{ row.description || '-' }}</span>
          </template>
        </el-table-column>

        <el-table-column label="Inputs" min-width="260">
          <template #default="{ row }">
            <div class="inputs-wrap">
              <template v-if="getInputsList(row).length">
                <span
                  v-for="(it, idx) in getInputsList(row)"
                  :key="idx"
                  class="input-chip"
                >
                  <span class="key" :title="it.key">
                    {{ it.key }}<span v-if="it.required" class="req">*</span>
                  </span>
                  <span class="type">{{ it.type }}</span>
                  <span v-if="it.extras" class="extras">{{ it.extras }}</span>
                </span>
              </template>
              <span v-else class="input-chip empty">None</span>
            </div>
          </template>
        </el-table-column>

        <el-table-column label="Actions" width="180" fixed="right" align="center">
          <template #default="{ row }">
            <div class="action-links">
              <span class="action-link" @click="viewToolDetails(row)">Detail</span>
              <span class="action-link" @click="executeTool(row)">Execute</span>
            </div>
          </template>
        </el-table-column>
      </el-table>

      <!-- 空状态 -->
      <div v-if="filteredTools.length === 0 && !loading" class="empty-container">
        <el-icon class="empty-icon"><Tools /></el-icon>
        <div class="empty-text">No tools available</div>
        <div class="empty-description">
          {{ searchQuery || serviceFilter ? 'No matching tools found' : 'No tools available yet' }}
        </div>
      </div>
    </el-card>

    <!-- 工具详情对话框 -->
    <el-dialog
      v-model="detailDialogVisible"
      :title="`Tool Detail - ${selectedTool?.name}`"
      width="600px"
    >
      <div v-if="selectedTool" class="tool-details">
        <el-descriptions :column="1" border>
          <el-descriptions-item label="Tool Name">
            {{ selectedTool.name }}
          </el-descriptions-item>
          <el-descriptions-item label="Service">
            {{ selectedTool.service }}
          </el-descriptions-item>
          <el-descriptions-item label="Description">
            {{ selectedTool.description || 'No description' }}
          </el-descriptions-item>
          <el-descriptions-item label="Parameters">
            {{ getInputsList(selectedTool).length }}
          </el-descriptions-item>
        </el-descriptions>

        <!-- 参数详情 -->
        <div v-if="selectedTool.input_schema" class="params-section">
          <h4>Input Schema</h4>
          <pre>{{ JSON.stringify(selectedTool.input_schema, null, 2) }}</pre>
        </div>
      </div>

      <template #footer>
        <el-button @click="detailDialogVisible = false">Close</el-button>
        <el-button type="primary" @click="executeTool(selectedTool)">Execute</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import { summarizeInputs, schemaToList } from '@/utils/schema'
import {
  Refresh, Tools, Connection, Menu, Search
} from '@element-plus/icons-vue'

const router = useRouter()
const route = useRoute()
const systemStore = useSystemStore()

// 响应式数据
const loading = ref(false)
const searchQuery = ref('')
const serviceFilter = ref('')
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

  // 服务过滤（直接匹配 service 字段）
  if (serviceFilter.value) {
    tools = tools.filter(tool => tool.service === serviceFilter.value)
  }

  return tools
})

const serviceNames = computed(() => {
  const names = new Set(systemStore.tools.map(tool => tool.service))
  return Array.from(names).sort()
})

const toolsByService = computed(() => systemStore.toolsByService)

// 方法
const refreshTools = async () => {
  loading.value = true
  try {
    await systemStore.fetchTools()
    ElMessage.success('Tools refreshed successfully')
  } catch (error) {
    ElMessage.error('Failed to refresh tools')
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

// 基于 JSON Schema 的输入参数工具函数
const getInputsSummary = (tool) => {
  try {
    return summarizeInputs(tool?.input_schema || {}) || 'None'
  } catch {
    return 'None'
  }
}

const getInputsList = (tool) => {
  try {
    return schemaToList(tool?.input_schema || {})
  } catch {
    return []
  }
}

const executeTool = (tool) => {
  router.push({
    path: '/for_store/call_tool',
    query: { 
      toolName: tool.name,
      serviceName: tool.service
    }
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
  width: 92%;
  margin: 0 auto;
  max-width: none;
  padding: 20px;

  .page-header {
    display: flex;
    justify-content: space-between;
    margin-bottom: 20px;

    .header-left {
      .page-title {
        margin: 0 0 4px 0;
        font-size: 24px;
        font-weight: 600;
      }

      .page-description {
        margin: 0;
        color: var(--el-text-color-secondary);
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
      box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
      padding: 20px;
      display: flex;
      align-items: center;
      gap: 16px;
      background: var(--el-bg-color);
      border-radius: 8px;

      .stat-icon {
        width: 50px;
        height: 50px;
        border-radius: 8px;
        display: flex;
        align-items: center;
        justify-content: center;
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
          font-weight: 600;
          color: var(--el-text-color-primary);
        }

        .stat-label {
          font-size: 14px;
          color: var(--el-text-color-secondary);
        }
      }
    }
  }

  .filter-card {
    margin-bottom: 20px;
  }

  .tools-card {
    .inputs-wrap {
      display: flex;
      gap: 6px;
      flex-wrap: wrap;
    }

    .input-chip {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 4px 8px;
      border-radius: 6px;
      background: var(--el-fill-color-light);
      border: 1px solid var(--el-border-color-lighter);
      color: var(--el-text-color-primary);
      font-size: 12px;

      .key {
        font-weight: 600;
      }

      .req {
        color: var(--el-color-danger);
        margin-left: 2px;
      }

      .type {
        color: var(--el-text-color-secondary);
      }

      .extras {
        color: var(--el-text-color-secondary);
      }

      &.empty {
        opacity: 0.7;
        font-style: italic;
      }
    }

    .action-links {
      display: flex;
      gap: 12px;
      justify-content: center;
    }

    .action-link {
      color: var(--el-color-primary);
      cursor: pointer;
      user-select: none;

      &:hover {
        text-decoration: underline;
      }
    }

    .empty-container {
      padding: 60px 20px;
      text-align: center;

      .empty-icon {
        font-size: 64px;
        color: var(--el-text-color-placeholder);
        margin-bottom: 16px;
      }

      .empty-text {
        font-size: 18px;
        color: var(--el-text-color-primary);
        margin-bottom: 8px;
      }

      .empty-description {
        color: var(--el-text-color-secondary);
        font-size: 14px;
      }
    }
  }

  .tool-details {
    .params-section {
      margin-top: 20px;

      h4 {
        margin-bottom: 12px;
        color: var(--el-text-color-primary);
      }

      pre {
        background: var(--el-fill-color-light);
        padding: 12px;
        border-radius: 8px;
        font-size: 14px;
        max-height: 200px;
        overflow-y: auto;
      }
    }
  }
}
</style>

