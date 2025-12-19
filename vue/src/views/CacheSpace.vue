<template>
  <div class="cache-space-container">
    <!-- Header -->
    <header class="page-header">
      <div class="header-content">
        <h1 class="page-title">
          Cache Inspector
        </h1>
        <p class="page-subtitle">
          Inspect and monitor system cache state
        </p>
      </div>
      <div class="header-actions">
        <el-button 
          :icon="Refresh" 
          :loading="loading" 
          circle
          plain
          class="refresh-btn"
          @click="refreshCache"
        />
      </div>
    </header>

    <!-- KPI Grid -->
    <div class="kpi-grid">
      <StatCard
        title="Total Keys"
        :value="totalKeys"
        unit="items"
        :icon="Coin"
        class="kpi-card"
      />
      <StatCard
        title="Service Keys"
        :value="serviceKeys"
        unit="svcs"
        :icon="Connection"
        class="kpi-card"
      />
      <StatCard
        title="Tool Keys"
        :value="toolKeys"
        unit="fns"
        :icon="Tools"
        class="kpi-card"
      />
    </div>

    <!-- Main Layout -->
    <div class="main-layout">
      <!-- Left Panel: Keys List -->
      <div class="panel-column left-col">
        <section class="panel-section full-height">
          <div class="panel-header">
            <h3 class="panel-title">
              Keys
            </h3>
            <div class="panel-controls">
              <div class="search-wrapper">
                <el-icon class="search-icon">
                  <Search />
                </el-icon>
                <input
                  v-model="searchQuery"
                  class="atom-input small"
                  placeholder="Search keys..."
                >
              </div>
            </div>
          </div>
          
          <!-- Category Tabs -->
          <div class="tabs-header">
            <div 
              v-for="cat in ['all', 'service', 'tool', 'config', 'status']" 
              :key="cat"
              class="tab-item" 
              :class="{ active: activeCategory === cat }"
              @click="activeCategory = cat"
            >
              {{ cat.charAt(0).toUpperCase() + cat.slice(1) }}
            </div>
          </div>

          <div class="panel-body keys-list">
            <div 
              v-for="key in filteredKeys" 
              :key="key.key" 
              class="key-item" 
              :class="{ active: selectedKey?.key === key.key }"
              @click="selectKey(key)"
            >
              <div class="key-icon">
                <el-icon><Key /></el-icon>
              </div>
              <div class="key-content">
                <span
                  class="key-name"
                  :title="key.key"
                >{{ key.key }}</span>
                <div class="key-meta">
                  <span :class="['tag', key.category]">{{ key.category }}</span>
                  <span class="type">{{ key.type }}</span>
                </div>
              </div>
            </div>
             
            <div
              v-if="filteredKeys.length === 0"
              class="empty-state small"
            >
              No keys found.
            </div>
          </div>
          <div class="panel-footer">
            <span>{{ filteredKeys.length }} keys</span>
          </div>
        </section>
      </div>

      <!-- Right Panel: Value Inspector -->
      <div class="panel-column right-col">
        <section class="panel-section full-height">
          <div
            v-if="selectedKey"
            class="value-inspector"
          >
            <div class="inspector-header">
              <div class="key-title">
                <h3>{{ selectedKey.key }}</h3>
                <div class="meta-badges">
                  <span class="badge">{{ selectedKey.type.toUpperCase() }}</span>
                  <span class="badge size">{{ formatSize(selectedKey.size) }}</span>
                  <span
                    class="badge ttl"
                    :class="{ infinite: selectedKey.ttl < 0 }"
                  >
                    {{ formatTTL(selectedKey.ttl) }}
                  </span>
                </div>
              </div>
              <div class="actions">
                <button
                  class="text-btn"
                  @click="copyValue"
                >
                  Copy
                </button>
                <button
                  class="text-btn"
                  @click="exportValue"
                >
                  Export
                </button>
              </div>
            </div>
              
            <div class="inspector-body">
              <!-- String/JSON -->
              <div
                v-if="['string', 'json', 'object'].includes(selectedKey.type)"
                class="code-view"
              >
                <textarea
                  readonly
                  class="code-editor"
                  :value="displayValue"
                />
              </div>
                 
              <!-- Hash -->
              <div
                v-else-if="selectedKey.type === 'hash'"
                class="table-view"
              >
                <table class="atom-table">
                  <thead>
                    <tr><th>FIELD</th><th>VALUE</th></tr>
                  </thead>
                  <tbody>
                    <tr
                      v-for="(row, idx) in hashTableData"
                      :key="idx"
                    >
                      <td class="mono">
                        {{ row.field }}
                      </td>
                      <td class="mono val">
                        {{ row.value }}
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>
                 
              <!-- List -->
              <div
                v-else-if="selectedKey.type === 'list'"
                class="table-view"
              >
                <table class="atom-table">
                  <thead><tr><th>INDEX</th><th>VALUE</th></tr></thead>
                  <tbody>
                    <tr
                      v-for="(row, idx) in listTableData"
                      :key="idx"
                    >
                      <td class="mono idx">
                        {{ idx }}
                      </td>
                      <td class="mono val">
                        {{ row.value }}
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>
                 
              <!-- Set -->
              <div
                v-else-if="selectedKey.type === 'set'"
                class="set-view"
              >
                <span
                  v-for="(item, idx) in selectedKey.value"
                  :key="idx"
                  class="set-item"
                >
                  {{ item }}
                </span>
              </div>
            </div>
              
            <div class="inspector-footer">
              <div class="time-info">
                <span>Created: {{ formatTime(selectedKey.created_at) }}</span>
                <span>Updated: {{ formatTime(selectedKey.updated_at) }}</span>
              </div>
            </div>
          </div>
           
          <div
            v-else
            class="empty-state"
          >
            <el-icon class="icon">
              <Coin />
            </el-icon>
            <p>Select a key to inspect value.</p>
          </div>
        </section>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import StatCard from '@/components/common/StatCard.vue'
import {
  Refresh, Coin, Connection, Tools, Search, Key
} from '@element-plus/icons-vue'

// State
const loading = ref(false)
const searchQuery = ref('')
const activeCategory = ref('all')
const selectedKey = ref(null)

// Mock Data (Preserved)
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
  if (activeCategory.value !== 'all') {
    keys = keys.filter(k => k.category === activeCategory.value)
  }
  if (searchQuery.value) {
    const q = searchQuery.value.toLowerCase()
    keys = keys.filter(k => k.key.toLowerCase().includes(q))
  }
  return keys
})

const totalKeys = computed(() => cacheKeys.value.length)
const serviceKeys = computed(() => cacheKeys.value.filter(k => k.category === 'service').length)
const toolKeys = computed(() => cacheKeys.value.filter(k => k.category === 'tool').length)

const displayValue = computed(() => {
  if (!selectedKey.value) return ''
  return typeof selectedKey.value.value === 'string' 
    ? selectedKey.value.value 
    : JSON.stringify(selectedKey.value.value, null, 2)
})

const hashTableData = computed(() => {
  if (selectedKey.value?.type !== 'hash') return []
  return Object.entries(selectedKey.value.value).map(([field, value]) => ({
    field,
    value: typeof value === 'object' ? JSON.stringify(value) : String(value)
  }))
})

const listTableData = computed(() => {
  if (selectedKey.value?.type !== 'list') return []
  return selectedKey.value.value.map(value => ({ value }))
})

// Methods
const refreshCache = async () => {
  loading.value = true
  try {
    await new Promise(r => setTimeout(r, 500))
    ElMessage.success('Refreshed')
  } catch (e) {
    ElMessage.error(e.message)
  } finally {
    loading.value = false
  }
}

const selectKey = (key) => selectedKey.value = key

const formatTTL = (seconds) => {
  if (seconds < 0) return '∞'
  if (seconds < 60) return `${seconds}s`
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`
  return `${Math.floor(seconds / 3600)}h`
}

const formatSize = (bytes) => {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(2)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`
}

const formatTime = (ts) => new Date(ts).toLocaleString()

const copyValue = async () => {
  try {
    await navigator.clipboard.writeText(displayValue.value)
    ElMessage.success('Copied')
  } catch {
    ElMessage.error('Copy failed')
  }
}

const exportValue = () => {
  if (!selectedKey.value) return
  const blob = new Blob([displayValue.value], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `${selectedKey.value.key.replace(/:/g, '_')}.json`
  a.click()
  URL.revokeObjectURL(url)
}

onMounted(() => {
  if (cacheKeys.value.length > 0) selectedKey.value = cacheKeys.value[0]
})
</script>

<style lang="scss" scoped>
.cache-space-container {
  max-width: 1400px;
  margin: 0 auto;
  padding: 20px;
  width: 100%;
}

// Header
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  margin-bottom: 24px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--border-color);
}

.page-title {
  font-size: 20px;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 4px;
}

.page-subtitle {
  font-size: 13px;
  color: var(--text-secondary);
}

.refresh-btn {
  border-color: var(--border-color);
  color: var(--text-secondary);
  &:hover { color: var(--text-primary); background: transparent; border-color: var(--text-secondary); }
}

// KPI
.kpi-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 16px;
  margin-bottom: 24px;
  
  @media (max-width: 768px) { grid-template-columns: 1fr; }
}

.kpi-card { height: 100%; }

// Layout
.main-layout {
  display: grid;
  grid-template-columns: 320px 1fr;
  gap: 24px;
  height: calc(100vh - 250px);
  min-height: 500px;
  
  @media (max-width: 900px) {
    grid-template-columns: 1fr;
    height: auto;
  }
}

.panel-column {
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.panel-section {
  display: flex;
  flex-direction: column;
  gap: 0; // Tighter layout
  &.full-height { height: 100%; }
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
  
  .panel-title {
    font-size: 13px;
    font-weight: 600;
    text-transform: uppercase;
    color: var(--text-secondary);
    letter-spacing: 0.05em;
  }
}

.panel-body {
  background: var(--bg-surface);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  overflow: hidden;
  flex: 1;
  display: flex;
  flex-direction: column;
}

.panel-footer {
  font-size: 11px;
  color: var(--text-placeholder);
  text-align: center;
  margin-top: 8px;
}

// Search & Tabs
.search-wrapper {
  position: relative;
  .search-icon {
    position: absolute;
    left: 8px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-placeholder);
    font-size: 12px;
  }
  .atom-input {
    border: 1px solid var(--border-color);
    background: var(--bg-surface);
    padding: 4px 8px 4px 24px;
    border-radius: 4px;
    font-size: 12px;
    color: var(--text-primary);
    width: 140px;
    &:focus { outline: none; border-color: var(--text-secondary); }
  }
}

.tabs-header {
  display: flex;
  gap: 4px;
  margin-bottom: 8px;
  flex-wrap: wrap;
  
  .tab-item {
    font-size: 11px;
    padding: 4px 8px;
    border-radius: 4px;
    color: var(--text-secondary);
    cursor: pointer;
    font-weight: 500;
    
    &:hover { background: var(--bg-hover); color: var(--text-primary); }
    &.active { background: var(--text-primary); color: var(--bg-surface); }
  }
}

// Keys List
.keys-list {
  overflow-y: auto;
  padding: 8px;
}

.key-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
  border: 1px solid transparent;
  
  &:hover { background: var(--bg-hover); }
  &.active { background: var(--bg-hover); border-color: var(--border-color); }
  
  .key-icon { font-size: 16px; color: var(--text-secondary); }
  
  .key-content {
    display: flex;
    flex-direction: column;
    min-width: 0;
    
    .key-name { 
      font-size: 13px; 
      font-weight: 500; 
      color: var(--text-primary); 
      white-space: nowrap; 
      overflow: hidden; 
      text-overflow: ellipsis; 
    }
    
    .key-meta {
      display: flex;
      align-items: center;
      gap: 6px;
      margin-top: 2px;
      
      .tag { 
        font-size: 9px; 
        text-transform: uppercase; 
        font-weight: 700;
        
        &.service { color: var(--color-accent); }
        &.tool { color: var(--color-success); }
        &.config { color: var(--color-warning); }
      }
      
      .type { font-size: 10px; color: var(--text-placeholder); }
    }
  }
}

// Value Inspector
.value-inspector {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.inspector-header {
  padding: 16px;
  border-bottom: 1px solid var(--border-color);
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  
  .key-title {
    h3 { margin: 0 0 8px; font-size: 16px; word-break: break-all; color: var(--text-primary); }
    .meta-badges {
      display: flex;
      gap: 8px;
      
      .badge {
        font-size: 10px;
        font-weight: 700;
        padding: 2px 6px;
        border-radius: 4px;
        background: var(--bg-hover);
        color: var(--text-secondary);
        
        &.ttl { 
           &.infinite { color: var(--color-success); background: #dcfce7; }
        }
      }
    }
  }
  
  .actions {
    display: flex;
    gap: 8px;
  }
}

.inspector-body {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  
  .code-view { flex: 1; position: relative; }
  .code-editor {
    width: 100%;
    height: 100%;
    border: none;
    resize: none;
    padding: 16px;
    font-family: var(--font-mono);
    font-size: 13px;
    line-height: 1.5;
    background: var(--bg-body);
    color: var(--text-primary);
    box-sizing: border-box;
    &:focus { outline: none; }
  }
  
  .table-view {
    flex: 1;
    overflow: auto;
    
    .atom-table {
      width: 100%;
      border-collapse: collapse;
      
      th { 
        text-align: left; 
        font-size: 11px; 
        color: var(--text-secondary); 
        padding: 8px 16px; 
        border-bottom: 1px solid var(--border-color); 
        background: var(--bg-hover);
      }
      
      td {
        padding: 8px 16px;
        border-bottom: 1px solid var(--border-color);
        font-size: 13px;
        color: var(--text-primary);
        &.mono { font-family: var(--font-mono); }
        &.idx { color: var(--text-placeholder); width: 60px; }
      }
    }
  }
  
  .set-view {
    padding: 16px;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    overflow: auto;
    
    .set-item {
      padding: 6px 10px;
      background: var(--bg-hover);
      border-radius: 4px;
      font-size: 13px;
      font-family: var(--font-mono);
      color: var(--text-primary);
    }
  }
}

.inspector-footer {
  padding: 12px 16px;
  border-top: 1px solid var(--border-color);
  background: var(--bg-hover);
  
  .time-info {
    display: flex;
    gap: 16px;
    font-size: 11px;
    color: var(--text-secondary);
  }
}

.text-btn {
  background: none;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  padding: 4px 8px;
  font-size: 11px;
  font-weight: 500;
  color: var(--text-secondary);
  cursor: pointer;
  &:hover { color: var(--text-primary); border-color: var(--text-primary); }
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--text-placeholder);
  
  .icon { font-size: 48px; margin-bottom: 16px; opacity: 0.5; }
  &.small { font-size: 12px; font-style: italic; }
}
</style>