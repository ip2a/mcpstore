<template>
  <div class="cache-space">
    <!-- Page Header -->
    <div class="page-header">
      <div class="header-content">
        <div class="header-title">
          <h1>缓存空间</h1>
          <p class="subtitle">查看和监控系统缓存数据（只读）</p>
        </div>
        <div class="header-actions">
          <el-button
            type="primary"
            :icon="Refresh"
            @click="refreshCache"
            :loading="loading"
          >
            刷新
          </el-button>
        </div>
      </div>
    </div>

    <!-- Stats Cards -->
    <el-row :gutter="20" class="stats-cards">
      <el-col :xs="24" :sm="8">
        <div class="stat-card">
          <div class="stat-icon cache">
            <el-icon size="24"><Coin /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ totalKeys }}</div>
            <div class="stat-label">缓存键总数</div>
          </div>
        </div>
      </el-col>
      <el-col :xs="24" :sm="8">
        <div class="stat-card">
          <div class="stat-icon service">
            <el-icon size="24"><Connection /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ serviceKeys }}</div>
            <div class="stat-label">服务缓存</div>
          </div>
        </div>
      </el-col>
      <el-col :xs="24" :sm="8">
        <div class="stat-card">
          <div class="stat-icon tool">
            <el-icon size="24"><Tools /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ toolKeys }}</div>
            <div class="stat-label">工具缓存</div>
          </div>
        </div>
      </el-col>
    </el-row>

    <!-- Main Content -->
    <el-row :gutter="20">
      <!-- Left Side: Cache Keys List -->
      <el-col :span="8">
        <el-card class="keys-card">
          <template #header>
            <div class="card-header">
              <span>缓存键列表</span>
              <el-input
                v-model="searchQuery"
                placeholder="搜索键名..."
                size="small"
                :prefix-icon="Search"
                clearable
              />
            </div>
          </template>
          
          <!-- Category Tabs -->
          <el-tabs v-model="activeCategory" @tab-change="handleCategoryChange">
            <el-tab-pane label="全部" name="all">
              <div class="keys-list">
                <div
                  v-for="key in filteredKeys"
                  :key="key.key"
                  class="key-item"
                  :class="{ active: selectedKey?.key === key.key }"
                  @click="selectKey(key)"
                >
                  <div class="key-info">
                    <div class="key-icon">
                      <el-icon><Key /></el-icon>
                    </div>
                    <div class="key-details">
                      <div class="key-name">{{ key.key }}</div>
                      <div class="key-meta">
                        <el-tag size="small" :type="getCategoryTag(key.category)">
                          {{ key.category }}
                        </el-tag>
                        <span class="key-type">{{ key.type }}</span>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </el-tab-pane>
            <el-tab-pane label="服务" name="service" />
            <el-tab-pane label="工具" name="tool" />
            <el-tab-pane label="配置" name="config" />
            <el-tab-pane label="状态" name="status" />
          </el-tabs>

          <template #footer>
            <div class="keys-footer">
              <span>共 {{ filteredKeys.length }} 个缓存键</span>
            </div>
          </template>
        </el-card>
      </el-col>

      <!-- Right Side: Cache Value Display -->
      <el-col :span="16">
        <el-card v-if="selectedKey" class="value-card">
          <template #header>
            <div class="value-header">
              <div class="header-info">
                <h3>{{ selectedKey.key }}</h3>
                <div class="header-tags">
                  <el-tag size="small" :type="getCategoryTag(selectedKey.category)">
                    {{ selectedKey.category }}
                  </el-tag>
                  <el-tag size="small" type="info">{{ selectedKey.type }}</el-tag>
                  <el-tag v-if="selectedKey.ttl > 0" size="small" type="warning">
                    TTL: {{ formatTTL(selectedKey.ttl) }}
                  </el-tag>
                  <el-tag v-else size="small" type="success">永久</el-tag>
                </div>
              </div>
              <div class="header-actions">
                <el-button
                  size="small"
                  :icon="CopyDocument"
                  @click="copyValue"
                >
                  复制值
                </el-button>
                <el-button
                  size="small"
                  :icon="Download"
                  @click="exportValue"
                >
                  导出
                </el-button>
              </div>
            </div>
          </template>

          <!-- Cache Value Display -->
          <div class="value-container">
            <el-descriptions :column="1" border class="value-meta">
              <el-descriptions-item label="键名">
                {{ selectedKey.key }}
              </el-descriptions-item>
              <el-descriptions-item label="类型">
                {{ selectedKey.type }}
              </el-descriptions-item>
              <el-descriptions-item label="分类">
                {{ selectedKey.category }}
              </el-descriptions-item>
              <el-descriptions-item label="大小">
                {{ formatSize(selectedKey.size) }}
              </el-descriptions-item>
              <el-descriptions-item label="创建时间">
                {{ formatTime(selectedKey.created_at) }}
              </el-descriptions-item>
              <el-descriptions-item label="更新时间">
                {{ formatTime(selectedKey.updated_at) }}
              </el-descriptions-item>
            </el-descriptions>

            <el-divider content-position="left">缓存值</el-divider>

            <!-- Value Display based on type -->
            <div class="value-display">
              <!-- String/JSON Display -->
              <el-input
                v-if="['string', 'json', 'object'].includes(selectedKey.type)"
                v-model="displayValue"
                type="textarea"
                :rows="20"
                readonly
                class="value-textarea"
              />

              <!-- Hash Display -->
              <el-table
                v-else-if="selectedKey.type === 'hash'"
                :data="hashTableData"
                stripe
                border
              >
                <el-table-column prop="field" label="字段" width="200" />
                <el-table-column prop="value" label="值" show-overflow-tooltip />
              </el-table>

              <!-- List Display -->
              <el-table
                v-else-if="selectedKey.type === 'list'"
                :data="listTableData"
                stripe
                border
              >
                <el-table-column type="index" label="索引" width="80" />
                <el-table-column prop="value" label="值" show-overflow-tooltip />
              </el-table>

              <!-- Set Display -->
              <div v-else-if="selectedKey.type === 'set'" class="set-display">
                <el-tag
                  v-for="(item, idx) in selectedKey.value"
                  :key="idx"
                  size="large"
                  class="set-tag"
                >
                  {{ item }}
                </el-tag>
              </div>
            </div>
          </div>
        </el-card>

        <!-- Empty State -->
        <el-card v-else class="empty-card">
          <el-empty description="请从左侧选择一个缓存键" />
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import {
  Refresh, Coin, Connection, Tools, Search, Key, CopyDocument, Download
} from '@element-plus/icons-vue'

// State
const loading = ref(false)
const searchQuery = ref('')
const activeCategory = ref('all')
const selectedKey = ref(null)

// Mock Cache Data (虚拟数据)
const cacheKeys = ref([
  {
    key: 'store:services:list',
    category: 'service',
    type: 'json',
    value: JSON.stringify({
      services: [
        { name: 'weather-api', status: 'healthy', tools_count: 5 },
        { name: 'local-tool', status: 'active', tools_count: 3 }
      ]
    }, null, 2),
    size: 2048,
    ttl: -1,
    created_at: Date.now() - 86400000,
    updated_at: Date.now() - 3600000
  },
  {
    key: 'store:tools:list',
    category: 'tool',
    type: 'json',
    value: JSON.stringify({
      tools: [
        { name: 'get_weather', service: 'weather-api', description: 'Get weather data' },
        { name: 'search_tool', service: 'local-tool', description: 'Search information' }
      ]
    }, null, 2),
    size: 1536,
    ttl: -1,
    created_at: Date.now() - 43200000,
    updated_at: Date.now() - 1800000
  },
  {
    key: 'service:weather-api:status',
    category: 'status',
    type: 'hash',
    value: {
      status: 'healthy',
      last_check: new Date().toISOString(),
      response_time: '123ms',
      consecutive_successes: 50
    },
    size: 512,
    ttl: 300,
    created_at: Date.now() - 7200000,
    updated_at: Date.now() - 60000
  },
  {
    key: 'config:mcp.json',
    category: 'config',
    type: 'json',
    value: JSON.stringify({
      mcpServers: {
        'sequential-thinking': {
          command: 'npx',
          args: ['-y', '@modelcontextprotocol/server-sequential-thinking']
        }
      }
    }, null, 2),
    size: 1024,
    ttl: -1,
    created_at: Date.now() - 172800000,
    updated_at: Date.now() - 7200000
  },
  {
    key: 'tool:records:recent',
    category: 'tool',
    type: 'list',
    value: [
      'get_weather: Beijing -> 22°C',
      'search_tool: AI -> 100 results',
      'get_weather: Shanghai -> 25°C'
    ],
    size: 768,
    ttl: 3600,
    created_at: Date.now() - 3600000,
    updated_at: Date.now() - 300000
  },
  {
    key: 'service:tags',
    category: 'service',
    type: 'set',
    value: ['weather', 'tool', 'api', 'mcp', 'stdio'],
    size: 256,
    ttl: -1,
    created_at: Date.now() - 86400000,
    updated_at: Date.now() - 86400000
  }
])

// Computed
const filteredKeys = computed(() => {
  let keys = cacheKeys.value

  // Category filter
  if (activeCategory.value !== 'all') {
    keys = keys.filter(k => k.category === activeCategory.value)
  }

  // Search filter
  if (searchQuery.value) {
    const query = searchQuery.value.toLowerCase()
    keys = keys.filter(k => k.key.toLowerCase().includes(query))
  }

  return keys
})

const totalKeys = computed(() => cacheKeys.value.length)
const serviceKeys = computed(() => cacheKeys.value.filter(k => k.category === 'service').length)
const toolKeys = computed(() => cacheKeys.value.filter(k => k.category === 'tool').length)

const displayValue = computed(() => {
  if (!selectedKey.value) return ''
  
  if (typeof selectedKey.value.value === 'string') {
    return selectedKey.value.value
  }
  
  return JSON.stringify(selectedKey.value.value, null, 2)
})

const hashTableData = computed(() => {
  if (!selectedKey.value || selectedKey.value.type !== 'hash') return []
  
  return Object.entries(selectedKey.value.value).map(([field, value]) => ({
    field,
    value: typeof value === 'object' ? JSON.stringify(value) : String(value)
  }))
})

const listTableData = computed(() => {
  if (!selectedKey.value || selectedKey.value.type !== 'list') return []
  
  return selectedKey.value.value.map(value => ({ value }))
})

// Methods
const refreshCache = async () => {
  loading.value = true
  try {
    // TODO: 调用实际的 API 接口获取缓存数据
    await new Promise(resolve => setTimeout(resolve, 500))
    ElMessage.success('缓存数据已刷新')
  } catch (error) {
    ElMessage.error('刷新失败: ' + error.message)
  } finally {
    loading.value = false
  }
}

const selectKey = (key) => {
  selectedKey.value = key
}

const handleCategoryChange = () => {
  selectedKey.value = null
}

const getCategoryTag = (category) => {
  const tags = {
    service: 'primary',
    tool: 'success',
    config: 'warning',
    status: 'info'
  }
  return tags[category] || 'info'
}

const formatTTL = (seconds) => {
  if (seconds < 0) return '永久'
  if (seconds < 60) return `${seconds}秒`
  if (seconds < 3600) return `${Math.floor(seconds / 60)}分钟`
  return `${Math.floor(seconds / 3600)}小时`
}

const formatSize = (bytes) => {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(2)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`
}

const formatTime = (timestamp) => {
  return new Date(timestamp).toLocaleString('zh-CN')
}

const copyValue = async () => {
  try {
    await navigator.clipboard.writeText(displayValue.value)
    ElMessage.success('已复制到剪贴板')
  } catch (error) {
    ElMessage.error('复制失败: ' + error.message)
  }
}

const exportValue = () => {
  if (!selectedKey.value) return

  const blob = new Blob([displayValue.value], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = `${selectedKey.value.key.replace(/:/g, '_')}.json`
  link.click()
  URL.revokeObjectURL(url)
  ElMessage.success('已导出')
}

// Lifecycle
onMounted(() => {
  // Auto select first key
  if (cacheKeys.value.length > 0) {
    selectedKey.value = cacheKeys.value[0]
  }
})
</script>

<style scoped>
.cache-space {
  padding: 20px;
}

.page-header {
  margin-bottom: 20px;
}

.header-content {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.header-title h1 {
  margin: 0 0 4px 0;
  font-size: 24px;
  font-weight: 600;
}

.subtitle {
  margin: 0;
  color: var(--el-text-color-secondary);
  font-size: 14px;
}

.header-actions {
  display: flex;
  gap: 12px;
}

/* Stats Cards */
.stats-cards {
  margin-bottom: 20px;
}

.stat-card {
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
  padding: 20px;
  display: flex;
  align-items: center;
  gap: 16px;
  background: var(--el-bg-color);
  border-radius: 8px;
}

.stat-icon {
  width: 50px;
  height: 50px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: white;
}

.stat-icon.cache {
  background: linear-gradient(135deg, #f093fb 0%, #f5576c 100%);
}

.stat-icon.service {
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
}

.stat-icon.tool {
  background: linear-gradient(135deg, #4facfe 0%, #00f2fe 100%);
}

.stat-content {
  flex: 1;
}

.stat-value {
  font-size: 24px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}

.stat-label {
  font-size: 14px;
  color: var(--el-text-color-secondary);
}

/* Keys Card */
.keys-card {
  min-height: 600px;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
}

.keys-list {
  max-height: 450px;
  overflow-y: auto;
}

.key-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
  margin-bottom: 8px;
}

.key-item:hover {
  background: var(--el-fill-color-light);
}

.key-item.active {
  background: var(--el-color-primary-light-9);
  border: 1px solid var(--el-color-primary);
}

.key-info {
  display: flex;
  align-items: center;
  gap: 12px;
  flex: 1;
}

.key-icon {
  font-size: 20px;
  color: var(--el-color-primary);
}

.key-details {
  flex: 1;
  min-width: 0;
}

.key-name {
  font-weight: 600;
  margin-bottom: 4px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.key-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
}

.key-type {
  color: var(--el-text-color-secondary);
}

.keys-footer {
  text-align: center;
  color: var(--el-text-color-secondary);
  font-size: 13px;
}

/* Value Card */
.value-card {
  min-height: 600px;
}

.value-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
}

.header-info h3 {
  margin: 0 0 8px 0;
  font-size: 16px;
  word-break: break-all;
}

.header-tags {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.header-actions {
  display: flex;
  gap: 8px;
}

.value-container {
  margin-top: 16px;
}

.value-meta {
  margin-bottom: 20px;
}

.value-display {
  margin-top: 12px;
}

.value-textarea :deep(textarea) {
  font-family: 'Courier New', Consolas, monospace;
  font-size: 13px;
  line-height: 1.6;
}

.set-display {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.set-tag {
  padding: 8px 12px;
}

.empty-card {
  min-height: 600px;
  display: flex;
  align-items: center;
  justify-content: center;
}
</style>

