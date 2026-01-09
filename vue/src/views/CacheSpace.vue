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
          :loading="loading.stats" 
          circle
          plain
          class="refresh-btn"
          @click="initialize"
        />
      </div>
    </header>

    <!-- KPI Grid -->
    <div class="kpi-grid">
      <StatCard
        title="Total Entities"
        :value="totalEntities"
        :icon="Coin"
        class="kpi-card"
      />
      <StatCard
        title="Total Relations"
        :value="totalRelations"
        :icon="Connection"
        class="kpi-card"
      />
       <StatCard
        title="Total States"
        :value="totalStates"
        :icon="Bell"
        class="kpi-card"
      />
    </div>

    <!-- Main Layout -->
    <div class="main-layout">
      <!-- Left Panel: Keys List -->
      <div class="panel-column left-col">
        <section class="panel-section full-height">
          <!-- Layer Tabs -->
          <div class="tabs-header layer-tabs">
            <div 
              v-for="layer in ['entities', 'relations', 'states']"
              :key="layer"
              class="tab-item" 
              :class="{ active: activeLayer === layer }"
              @click="activeLayer = layer"
            >
              {{ layer.charAt(0).toUpperCase() + layer.slice(1) }}
            </div>
          </div>
          
          <!-- Type Filters -->
          <div class="tabs-header type-filters">
            <div
              class="tab-item small"
              :class="{ active: activeType === 'all' }"
              @click="activeType = 'all'"
            >
              All
            </div>
            <div 
              v-for="type in layerTypes" 
              :key="type"
              class="tab-item small" 
              :class="{ active: activeType === type }"
              @click="activeType = type"
            >
              {{ type }} ({{ stats[activeLayer]?.[type] || 0 }})
            </div>
          </div>
          
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
                  placeholder="Filter by key..."
                  @input="handleSearch"
                >
              </div>
            </div>
          </div>

          <div class="panel-body keys-list">
            <div 
              v-for="key in keysList" 
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
                  <span class="type">{{ key.originalType }}</span>
                </div>
              </div>
            </div>
             
            <div
              v-if="keysList.length === 0 && !loading.list"
              class="empty-state small"
            >
              No keys found.
            </div>
             <div
              v-if="loading.list"
              class="empty-state small"
            >
              Loading...
            </div>
          </div>
          <div class="panel-footer">
            <span>Showing {{ keysList.length }} keys</span>
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
                  <span class="badge">{{ (selectedKey.type || 'unknown').toUpperCase() }}</span>
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
import { ref, computed, onMounted, watch } from 'vue'
import { ElMessage } from 'element-plus'
import StatCard from '@/components/common/StatCard.vue'
import { cacheApi } from '@/api/cache'
import { debounce } from 'lodash-es'
import {
  Refresh, Coin, Connection, Bell, Search, Key
} from '@element-plus/icons-vue'

// State
const loading = ref({ list: false, stats: false })
const searchQuery = ref('')
const activeLayer = ref('entities')
const activeType = ref('all')
const keysList = ref([])
const selectedKey = ref(null)
const stats = ref({}) // To store counts from inspect API

// Computed: KPI Totals
const totalEntities = computed(() => Object.values(stats.value.entities || {}).reduce((a, b) => a + b, 0))
const totalRelations = computed(() => Object.values(stats.value.relations || {}).reduce((a, b) => a + b, 0))
const totalStates = computed(() => Object.values(stats.value.states || {}).reduce((a, b) => a + b, 0))

// Computed: Dynamic Types for selected layer
const layerTypes = computed(() => {
  if (stats.value && stats.value[activeLayer.value]) {
    return Object.keys(stats.value[activeLayer.value])
  }
  return []
})

// Computed: Value Display
const displayValue = computed(() => {
  if (!selectedKey.value) return ''
  const { value } = selectedKey.value
  return typeof value === 'string' ? value : JSON.stringify(value, null, 2)
})

// --- Methods ---

// Initialization
const initialize = async () => {
  loading.value.stats = true
  try {
    const res = await cacheApi.inspect()
    const payload = res?.data
    console.log('Cache Inspector Initialized:', payload) // Debug log
    // TODO: 后续统一在 cacheApi 中解包响应，避免在视图层手动访问 Axios 响应
    if (payload?.success && payload.data?.counts) {
      stats.value = payload.data.counts
      ElMessage.success('Stats refreshed')
      // Trigger initial data fetch
      if (!layerTypes.value.includes(activeType.value)) {
        activeType.value = 'all'
      } else {
        fetchData() // Re-fetch if type still valid
      }
    } else {
      console.warn('Inspect API returned invalid data:', payload)
      stats.value = {}
    }
  } catch (e) {
    ElMessage.error(e.message || 'Failed to load stats')
    console.error('Inspect API Error:', e)
    stats.value = {}
  } finally {
    loading.value.stats = false
  }
}

// Data Fetching
const fetchData = async () => {
  loading.value.list = true
  selectedKey.value = null
  keysList.value = []
  
  try {
    let res
    const params = {}
    if (searchQuery.value) params.key = searchQuery.value
    if (activeType.value !== 'all') params.type = activeType.value

    const apiMap = {
      entities: cacheApi.getEntities,
      relations: cacheApi.getRelations,
      states: cacheApi.getStates
    }

    const apiMethod = apiMap[activeLayer.value]
    if (apiMethod) {
      res = await apiMethod(params)
    }

    const payload = res?.data
    // TODO: 后续统一在 cacheApi 中解包响应，减少以下重复逻辑
    if (payload?.success) {
      const items = payload.data?.items || []
      keysList.value = items.map(item => {
        const { _key, _type, ...value } = item
        return {
          key: _key || `unknown:${Math.random()}`,
          originalType: _type,
          category: _type?.split('_')[0] || 'unknown',
          type: typeof value === 'object' ? 'json' : 'string',
          value,
          size: JSON.stringify(value).length,
          ttl: item.ttl || -1,
          created_at: item.created_at,
          updated_at: item.updated_at
        }
      })
    }
  } catch (e) {
    ElMessage.error(e.message || 'Failed to load data')
    console.error(e)
  } finally {
    loading.value.list = false
  }
}

// Event Handlers
const handleSearch = debounce(() => {
  fetchData()
}, 300)

const selectKey = (key) => {
  selectedKey.value = key
}

// Watchers
watch(activeLayer, (newLayer, oldLayer) => {
  if (newLayer !== oldLayer) {
    activeType.value = 'all' // Reset to 'all' when layer changes
    fetchData() // Explicitly fetch data for the new layer
  }
})

watch(activeType, () => {
  searchQuery.value = '' // Reset search on type change
  fetchData()
})

// Lifecycle
onMounted(() => {
  initialize()
})

// --- Utility Functions ---
const formatTTL = (seconds) => {
  if (seconds === null || seconds === undefined || seconds < 0) return '∞'
  if (seconds < 60) return `${seconds}s`
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`
  return `${Math.floor(seconds / 3600)}h`
}

const formatSize = (bytes) => {
  if (!bytes) return '0 B'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(2)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`
}

const formatTime = (ts) => {
  if (!ts) return '-'
  return new Date(ts).toLocaleString()
}

const copyValue = async () => {
  if (!selectedKey.value) return
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
  height: calc(100vh - 320px); // Adjusted height
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
  gap: 0;
  &.full-height { height: 100%; }
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
  border-top: 1px solid var(--border-color);
  padding-top: 12px;
  
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
  
  &.layer-tabs {
    margin-bottom: 16px;
    .tab-item {
      font-size: 14px;
      padding: 6px 12px;
      font-weight: 600;
    }
  }

  &.type-filters {
    margin-bottom: 12px;
    .tab-item {
      font-size: 11px;
      padding: 4px 8px;
      font-weight: 500;
      &.small { // ensure consistent naming if needed
        font-size: 11px;
        padding: 4px 8px;
        font-weight: 500;
      }
    }
  }
  
  .tab-item {
    border-radius: 6px;
    color: var(--text-secondary);
    cursor: pointer;
    transition: all 0.2s;
    
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
        
        &.service, &.entities { color: var(--color-accent); }
        &.tool { color: var(--color-success); }
        &.config, &.states { color: var(--color-warning); }
        &.relations { color: #9333ea; } // A new color for relations
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
