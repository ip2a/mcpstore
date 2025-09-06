<template>
  <div class="advanced-search">
    <!-- Search Header -->
    <div class="search-header">
      <div class="search-input-container">
        <el-input
          v-model="searchQuery"
          placeholder="搜索服务、工具、代理..."
          :prefix-icon="Search"
          clearable
          size="large"
          class="main-search-input"
          @keyup.enter="handleSearch"
          @clear="handleClear"
        >
          <template #append>
            <el-button 
              type="primary" 
              :icon="Search"
              @click="handleSearch"
              :loading="isSearching"
            >
              搜索
            </el-button>
          </template>
        </el-input>
      </div>
      
      <div class="search-actions">
        <el-button 
          text 
          :icon="Filter"
          @click="showFilters = !showFilters"
          :class="{ 'active': hasActiveFilters }"
        >
          筛选
          <el-badge 
            v-if="activeFiltersCount > 0" 
            :value="activeFiltersCount" 
            class="filter-badge"
          />
        </el-button>
        
        <el-dropdown @command="handleSortCommand" trigger="click">
          <el-button text>
            排序
            <el-icon class="el-icon--right"><ArrowDown /></el-icon>
          </el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item 
                v-for="option in sortOptions"
                :key="option.value"
                :command="option.value"
                :class="{ 'active': currentSort === option.value }"
              >
                {{ option.label }}
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
        
        <el-button text @click="saveSearch" :disabled="!searchQuery && !hasActiveFilters">
          <el-icon><Star /></el-icon>
          保存搜索
        </el-button>
      </div>
    </div>

    <!-- Advanced Filters Panel -->
    <el-collapse-transition>
      <div v-show="showFilters" class="filters-panel">
        <el-card class="filters-card">
          <template #header>
            <div class="filters-header">
              <span>高级筛选</span>
              <el-button text @click="resetFilters" size="small">重置</el-button>
            </div>
          </template>
          
          <div class="filters-content">
            <!-- Entity Type Filter -->
            <el-form-item label="实体类型">
              <el-checkbox-group v-model="filters.entityTypes">
                <el-checkbox label="services">服务</el-checkbox>
                <el-checkbox label="tools">工具</el-checkbox>
                <el-checkbox label="agents">代理</el-checkbox>
              </el-checkbox-group>
            </el-form-item>
            
            <!-- Status Filter -->
            <el-form-item label="状态">
              <el-select
                v-model="filters.status"
                multiple
                collapse-tags
                collapse-tags-tooltip
                placeholder="选择状态..."
                style="width: 100%"
              >
                <el-option label="健康" value="healthy" />
                <el-option label="活跃" value="active" />
                <el-option label="警告" value="warning" />
                <el-option label="错误" value="error" />
                <el-option label="离线" value="offline" />
              </el-select>
            </el-form-item>
            
            <!-- Date Range Filter -->
            <el-form-item label="创建时间">
              <el-date-picker
                v-model="filters.dateRange"
                type="daterange"
                range-separator="至"
                start-placeholder="开始日期"
                end-placeholder="结束日期"
                format="YYYY-MM-DD"
                value-format="YYYY-MM-DD"
                style="width: 100%"
              />
            </el-form-item>
            
            <!-- Tags Filter -->
            <el-form-item label="标签">
              <el-select
                v-model="filters.tags"
                multiple
                filterable
                allow-create
                default-first-option
                placeholder="输入或选择标签..."
                style="width: 100%"
              >
                <el-option
                  v-for="tag in availableTags"
                  :key="tag"
                  :label="tag"
                  :value="tag"
                />
              </el-select>
            </el-form-item>
            
            <!-- Custom Attributes -->
            <el-form-item label="自定义属性">
              <div class="custom-attributes">
                <div 
                  v-for="(attr, index) in filters.customAttributes"
                  :key="index"
                  class="attribute-item"
                >
                  <el-input
                    v-model="attr.key"
                    placeholder="属性名"
                    style="width: 120px"
                  />
                  <el-select v-model="attr.operator" style="width: 100px">
                    <el-option label="等于" value="eq" />
                    <el-option label="包含" value="contains" />
                    <el-option label="大于" value="gt" />
                    <el-option label="小于" value="lt" />
                  </el-select>
                  <el-input
                    v-model="attr.value"
                    placeholder="值"
                    style="width: 150px"
                  />
                  <el-button 
                    text 
                    :icon="Delete"
                    @click="removeAttribute(index)"
                  />
                </div>
                <el-button text @click="addAttribute" :icon="Plus">
                  添加属性
                </el-button>
              </div>
            </el-form-item>
          </div>
          
          <template #footer>
            <div class="filters-footer">
              <el-button @click="showFilters = false">取消</el-button>
              <el-button type="primary" @click="applyFilters">应用筛选</el-button>
            </div>
          </template>
        </el-card>
      </div>
    </el-collapse-transition>

    <!-- Search Results -->
    <div v-if="showResults" class="search-results">
      <!-- Results Summary -->
      <div class="results-summary">
        <div class="summary-info">
          <span class="results-count">找到 {{ totalResults }} 个结果</span>
          <span v-if="searchTime" class="search-time">
            (搜索耗时: {{ searchTime }}ms)
          </span>
        </div>
        
        <div class="results-actions">
          <el-button-group>
            <el-button 
              :type="viewMode === 'list' ? 'primary' : 'default'"
              :icon="List"
              @click="viewMode = 'list'"
              size="small"
            >
              列表
            </el-button>
            <el-button 
              :type="viewMode === 'grid' ? 'primary' : 'default'"
              :icon="Grid"
              @click="viewMode = 'grid'"
              size="small"
            >
              网格
            </el-button>
          </el-button-group>
          
          <el-dropdown @command="handleExportCommand" trigger="click">
            <el-button size="small">
              导出结果
              <el-icon class="el-icon--right"><ArrowDown /></el-icon>
            </el-button>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item command="json">JSON格式</el-dropdown-item>
                <el-dropdown-item command="csv">CSV格式</el-dropdown-item>
                <el-dropdown-item command="excel">Excel格式</el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </div>
      </div>

      <!-- Results List View -->
      <div v-if="viewMode === 'list'" class="results-list">
        <div v-if="results.length === 0" class="empty-results">
          <el-icon><Box /></el-icon>
          <p>未找到匹配的结果</p>
          <el-button @click="resetSearch">重新搜索</el-button>
        </div>
        
        <div v-else class="result-items">
          <div 
            v-for="item in results" 
            :key="item.id"
            class="result-item"
            :class="item.type"
            @click="handleItemClick(item)"
          >
            <div class="item-icon">
              <el-icon v-if="item.type === 'services'"><Connection /></el-icon>
              <el-icon v-else-if="item.type === 'tools'"><Tools /></el-icon>
              <el-icon v-else-if="item.type === 'agents'"><User /></el-icon>
            </div>
            
            <div class="item-content">
              <div class="item-title">{{ item.title }}</div>
              <div class="item-description">{{ item.description }}</div>
              <div class="item-meta">
                <el-tag size="small" :type="getStatusType(item.status)">
                  {{ item.status }}
                </el-tag>
                <span class="item-date">{{ formatDate(item.createdAt) }}</span>
              </div>
            </div>
            
            <div class="item-actions">
              <el-button text size="small" @click.stop="viewDetails(item)">
                查看详情
              </el-button>
              <el-dropdown @command="(cmd) => handleItemAction(cmd, item)" trigger="click">
                <el-button text size="small">
                  <el-icon><MoreFilled /></el-icon>
                </el-button>
                <template #dropdown>
                  <el-dropdown-menu>
                    <el-dropdown-item command="edit">编辑</el-dropdown-item>
                    <el-dropdown-item command="copy">复制</el-dropdown-item>
                    <el-dropdown-item command="delete" divided>删除</el-dropdown-item>
                  </el-dropdown-menu>
              </template>
              </el-dropdown>
            </div>
          </div>
        </div>
      </div>

      <!-- Results Grid View -->
      <div v-else class="results-grid">
        <el-row :gutter="20">
          <el-col 
            v-for="item in results"
            :key="item.id"
            :xs="24" :sm="12" :md="8" :lg="6"
          >
            <el-card 
              class="result-card"
              :class="item.type"
              shadow="hover"
              @click="handleItemClick(item)"
            >
              <div class="card-icon">
                <el-icon size="32">
                  <Connection v-if="item.type === 'services'" />
                  <Tools v-else-if="item.type === 'tools'" />
                  <User v-else-if="item.type === 'agents'" />
                </el-icon>
              </div>
              
              <div class="card-content">
                <h3>{{ item.title }}</h3>
                <p>{{ item.description }}</p>
                
                <div class="card-footer">
                  <el-tag size="small" :type="getStatusType(item.status)">
                    {{ item.status }}
                  </el-tag>
                  <span class="card-date">{{ formatDate(item.createdAt) }}</span>
                </div>
              </div>
            </el-card>
          </el-col>
        </el-row>
      </div>

      <!-- Pagination -->
      <div v-if="totalResults > pageSize" class="results-pagination">
        <el-pagination
          v-model:current-page="currentPage"
          v-model:page-size="pageSize"
          :page-sizes="[10, 20, 50, 100]"
          :total="totalResults"
          layout="total, sizes, prev, pager, next, jumper"
          @size-change="handleSizeChange"
          @current-change="handleCurrentChange"
        />
      </div>
    </div>

    <!-- Saved Searches Dialog -->
    <el-dialog
      v-model="savedSearchesDialogVisible"
      title="保存的搜索"
      width="600px"
    >
      <div class="saved-searches">
        <div v-if="savedSearches.length === 0" class="empty-saved">
          <el-icon><Star /></el-icon>
          <p>暂无保存的搜索</p>
        </div>
        
        <div v-else class="saved-searches-list">
          <div 
            v-for="search in savedSearches"
            :key="search.id"
            class="saved-search-item"
          >
            <div class="saved-search-info">
              <div class="saved-search-name">{{ search.name }}</div>
              <div class="saved-search-query">{{ search.query }}</div>
              <div class="saved-search-meta">
                <span>{{ formatDate(search.createdAt) }}</span>
                <el-tag size="small" v-if="search.isDefault">默认</el-tag>
              </div>
            </div>
            
            <div class="saved-search-actions">
              <el-button text @click="loadSavedSearch(search)">加载</el-button>
              <el-button text @click="editSavedSearch(search)">编辑</el-button>
              <el-button text @click="deleteSavedSearch(search)">删除</el-button>
            </div>
          </div>
        </div>
      </div>
      
      <template #footer>
        <el-button @click="savedSearchesDialogVisible = false">关闭</el-button>
        <el-button type="primary" @click="createNewSavedSearch">新建</el-button>
      </template>
    </el-dialog>

    <!-- Save Search Dialog -->
    <el-dialog
      v-model="saveSearchDialogVisible"
      title="保存搜索"
      width="400px"
    >
      <el-form :model="newSavedSearch" label-width="80px">
        <el-form-item label="名称">
          <el-input v-model="newSavedSearch.name" placeholder="输入搜索名称" />
        </el-form-item>
        <el-form-item label="设为默认">
          <el-switch v-model="newSavedSearch.isDefault" />
        </el-form-item>
      </el-form>
      
      <template #footer>
        <el-button @click="saveSearchDialogVisible = false">取消</el-button>
        <el-button type="primary" @click="confirmSaveSearch">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useSystemStore } from '@/stores/system'
import {
  Search, Filter, Star, ArrowDown, List, Grid, Delete, Plus,
  Connection, Tools, User, MoreFilled, Box
} from '@element-plus/icons-vue'

const router = useRouter()
const systemStore = useSystemStore()

// Reactive Data
const searchQuery = ref('')
const showFilters = ref(false)
const showResults = ref(false)
const isSearching = ref(false)
const viewMode = ref('list')
const currentPage = ref(1)
const pageSize = ref(20)
const totalResults = ref(0)
const searchTime = ref(0)

const filters = ref({
  entityTypes: [],
  status: [],
  dateRange: null,
  tags: [],
  customAttributes: []
})

const currentSort = ref('relevance')
const results = ref([])
const savedSearches = ref([])
const savedSearchesDialogVisible = ref(false)
const saveSearchDialogVisible = ref(false)
const newSavedSearch = ref({
  name: '',
  isDefault: false
})

// Sort Options
const sortOptions = [
  { label: '相关性', value: 'relevance' },
  { label: '创建时间', value: 'created' },
  { label: '更新时间', value: 'updated' },
  { label: '名称', value: 'name' },
  { label: '状态', value: 'status' }
]

// Available Tags (dynamic based on services)
const availableTags = computed(() => {
  const tags = new Set()
  
  // Extract potential tags from service descriptions
  if (systemStore.services && Array.isArray(systemStore.services)) {
    systemStore.services.forEach(service => {
      if (service.description) {
        // Simple tag extraction - in real implementation, you might have actual tags
        const words = service.description.split(/[,，\s]+/)
        words.forEach(word => {
          if (word.length > 1 && word.length < 10) {
            tags.add(word)
          }
        })
      }
    })
  }
  
  // Add some default tags
  ['重要', '开发', '生产', '测试', '监控', '安全', '性能', '工具'].forEach(tag => {
    tags.add(tag)
  })
  
  return Array.from(tags).sort()
})

// Computed Properties
const hasActiveFilters = computed(() => {
  return filters.value.entityTypes.length > 0 ||
         filters.value.status.length > 0 ||
         filters.value.dateRange ||
         filters.value.tags.length > 0 ||
         filters.value.customAttributes.length > 0
})

const activeFiltersCount = computed(() => {
  let count = 0
  count += filters.value.entityTypes.length
  count += filters.value.status.length
  count += filters.value.dateRange ? 1 : 0
  count += filters.value.tags.length
  count += filters.value.customAttributes.length
  return count
})

// Methods
const handleSearch = async () => {
  if (!searchQuery.value.trim() && !hasActiveFilters.value) {
    ElMessage.warning('请输入搜索关键词或设置筛选条件')
    return
  }

  isSearching.value = true
  const startTime = Date.now()
  
  try {
    // Fetch data if not already loaded
    if (systemStore.services.length === 0) {
      await systemStore.fetchServices()
    }
    if (systemStore.tools.length === 0) {
      await systemStore.fetchTools()
    }
    if (systemStore.agents.length === 0) {
      await systemStore.fetchAgents()
    }
    
    // Generate search results
    const allResults = generateSearchResults()
    
    // Apply search query filter
    let filteredResults = allResults
    if (searchQuery.value.trim()) {
      const query = searchQuery.value.toLowerCase()
      filteredResults = allResults.filter(item => 
        item.title.toLowerCase().includes(query) ||
        item.description.toLowerCase().includes(query)
      )
    }
    
    // Apply status filter
    if (filters.value.status.length > 0) {
      filteredResults = filteredResults.filter(item => 
        filters.value.status.includes(item.status)
      )
    }
    
    // Apply date range filter
    if (filters.value.dateRange && filters.value.dateRange.length === 2) {
      const startDate = new Date(filters.value.dateRange[0])
      const endDate = new Date(filters.value.dateRange[1])
      endDate.setHours(23, 59, 59, 999)
      
      filteredResults = filteredResults.filter(item => {
        const itemDate = new Date(item.createdAt)
        return itemDate >= startDate && itemDate <= endDate
      })
    }
    
    // Apply tags filter
    if (filters.value.tags.length > 0) {
      filteredResults = filteredResults.filter(item => {
        // This would require actual tag data in the items
        return filters.value.tags.some(tag => 
          item.description.toLowerCase().includes(tag.toLowerCase())
        )
      })
    }
    
    // Apply custom attributes filter
    if (filters.value.customAttributes.length > 0) {
      filteredResults = filteredResults.filter(item => {
        return filters.value.customAttributes.every(attr => {
          if (!attr.key || !attr.value) return true
          
          // Check if the item has the attribute and matches the condition
          const itemValue = item.data[attr.key]
          if (itemValue === undefined) return false
          
          switch (attr.operator) {
            case 'eq':
              return itemValue == attr.value
            case 'contains':
              return String(itemValue).toLowerCase().includes(String(attr.value).toLowerCase())
            case 'gt':
              return Number(itemValue) > Number(attr.value)
            case 'lt':
              return Number(itemValue) < Number(attr.value)
            default:
              return true
          }
        })
      })
    }
    
    // Apply sorting
    filteredResults.sort((a, b) => {
      switch (currentSort.value) {
        case 'created':
          return new Date(b.createdAt) - new Date(a.createdAt)
        case 'updated':
          return new Date(b.updatedAt || b.createdAt) - new Date(a.updatedAt || a.createdAt)
        case 'name':
          return a.title.localeCompare(b.title)
        case 'status':
          return a.status.localeCompare(b.status)
        default: // relevance
          // Calculate relevance score based on search query
          if (!searchQuery.value.trim()) return 0
          const query = searchQuery.value.toLowerCase()
          const aScore = (a.title.toLowerCase().includes(query) ? 2 : 0) + 
                        (a.description.toLowerCase().includes(query) ? 1 : 0)
          const bScore = (b.title.toLowerCase().includes(query) ? 2 : 0) + 
                        (b.description.toLowerCase().includes(query) ? 1 : 0)
          return bScore - aScore
      }
    })
    
    // Apply pagination
    const startIndex = (currentPage.value - 1) * pageSize.value
    const endIndex = startIndex + pageSize.value
    results.value = filteredResults.slice(startIndex, endIndex)
    totalResults.value = filteredResults.length
    searchTime.value = Date.now() - startTime
    showResults.value = true
  } catch (error) {
    ElMessage.error('搜索失败')
    console.error('Search error:', error)
  } finally {
    isSearching.value = false
  }
}

const handleClear = () => {
  searchQuery.value = ''
  showResults.value = false
  results.value = []
}

const resetFilters = () => {
  filters.value = {
    entityTypes: [],
    status: [],
    dateRange: null,
    tags: [],
    customAttributes: []
  }
}

const applyFilters = () => {
  showFilters.value = false
  handleSearch()
}

const handleSortCommand = (command) => {
  currentSort.value = command
  // Apply sorting logic
  handleSearch()
}

const handleExportCommand = (command) => {
  // Export logic based on format
  ElMessage.success(`导出为 ${command.toUpperCase()} 格式`)
}

const handleItemClick = (item) => {
  // Navigate to item detail page based on type
  switch (item.type) {
    case 'services':
      router.push(`/services/detail/${item.data.name}`)
      break
    case 'tools':
      router.push({
        path: '/tools/execute',
        query: { tool: item.data.name }
      })
      break
    case 'agents':
      router.push(`/agents/${item.data.id}/detail`)
      break
  }
}

const viewDetails = (item) => {
  handleItemClick(item)
}

const handleItemAction = (command, item) => {
  switch (command) {
    case 'edit':
      router.push(`/${item.type}/edit/${item.id}`)
      break
    case 'copy':
      // Copy item logic
      ElMessage.success('已复制')
      break
    case 'delete':
      deleteItem(item)
      break
  }
}

const deleteItem = async (item) => {
  try {
    await ElMessageBox.confirm(
      `确定要删除 "${item.title}" 吗？`,
      '删除确认',
      { type: 'warning' }
    )
    ElMessage.success('删除成功')
    // Refresh results
    handleSearch()
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error('删除失败')
    }
  }
}

const saveSearch = () => {
  saveSearchDialogVisible.value = true
}

const confirmSaveSearch = () => {
  if (!newSavedSearch.value.name.trim()) {
    ElMessage.warning('请输入搜索名称')
    return
  }

  const search = {
    id: Date.now(),
    name: newSavedSearch.value.name,
    query: searchQuery.value,
    filters: { ...filters.value },
    isDefault: newSavedSearch.value.isDefault,
    createdAt: new Date()
  }

  savedSearches.value.push(search)
  saveSearchDialogVisible.value = false
  newSavedSearch.value = { name: '', isDefault: false }
  ElMessage.success('搜索已保存')
}

const loadSavedSearch = (search) => {
  searchQuery.value = search.query
  filters.value = { ...search.filters }
  savedSearchesDialogVisible.value = false
  handleSearch()
}

const editSavedSearch = (search) => {
  // Edit logic
  ElMessage.info('编辑功能开发中')
}

const deleteSavedSearch = async (search) => {
  try {
    await ElMessageBox.confirm('确定要删除这个保存的搜索吗？')
    const index = savedSearches.value.findIndex(s => s.id === search.id)
    if (index > -1) {
      savedSearches.value.splice(index, 1)
      ElMessage.success('删除成功')
    }
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error('删除失败')
    }
  }
}

const createNewSavedSearch = () => {
  savedSearchesDialogVisible.value = false
  saveSearchDialogVisible.value = true
}

const resetSearch = () => {
  searchQuery.value = ''
  resetFilters()
  showResults.value = false
}

const addAttribute = () => {
  filters.value.customAttributes.push({
    key: '',
    operator: 'eq',
    value: ''
  })
}

const removeAttribute = (index) => {
  filters.value.customAttributes.splice(index, 1)
}

const handleSizeChange = (size) => {
  pageSize.value = size
  if (showResults.value) {
    handleSearch()
  }
}

const handleCurrentChange = (page) => {
  currentPage.value = page
  if (showResults.value) {
    handleSearch()
  }
}

const getStatusType = (status) => {
  const statusMap = {
    healthy: 'success',
    active: 'primary',
    warning: 'warning',
    error: 'danger',
    offline: 'info'
  }
  return statusMap[status] || 'info'
}

const formatDate = (date) => {
  if (!date) return '-'
  return new Date(date).toLocaleDateString('zh-CN')
}

// Generate search results from real data
const generateSearchResults = () => {
  const results = []
  
  // Add services
  if (filters.value.entityTypes.length === 0 || filters.value.entityTypes.includes('services')) {
    if (systemStore.services && Array.isArray(systemStore.services)) {
      systemStore.services.forEach(service => {
        results.push({
          id: `service-${service.name}`,
          type: 'services',
          title: service.name,
          description: service.description || `服务: ${service.url || service.command}`,
          status: service.status || 'unknown',
          createdAt: service.created_at || new Date(),
          data: service
        })
      })
    }
  }
  
  // Add tools
  if (filters.value.entityTypes.length === 0 || filters.value.entityTypes.includes('tools')) {
    if (systemStore.tools && Array.isArray(systemStore.tools)) {
      systemStore.tools.forEach(tool => {
        results.push({
          id: `tool-${tool.name}`,
          type: 'tools',
          title: tool.name,
          description: tool.description || '工具描述',
          status: 'active',
          createdAt: new Date(),
          data: tool
        })
      })
    }
  }
  
  // Add agents
  if (filters.value.entityTypes.length === 0 || filters.value.entityTypes.includes('agents')) {
    if (systemStore.agents && Array.isArray(systemStore.agents)) {
      systemStore.agents.forEach(agent => {
        results.push({
          id: `agent-${agent.id}`,
          type: 'agents',
          title: agent.name || agent.id,
          description: agent.description || 'Agent描述',
          status: agent.status || 'unknown',
          createdAt: agent.created_at || new Date(),
          data: agent
        })
      })
    }
  }
  
  return results
}

// Lifecycle
onMounted(async () => {
  // Load saved searches from localStorage
  const saved = localStorage.getItem('savedSearches')
  if (saved) {
    savedSearches.value = JSON.parse(saved)
  }
  
  // Pre-load data for better search experience
  if (systemStore.services.length === 0) {
    systemStore.fetchServices().catch(console.error)
  }
  if (systemStore.tools.length === 0) {
    systemStore.fetchTools().catch(console.error)
  }
  if (systemStore.agents.length === 0) {
    systemStore.fetchAgents().catch(console.error)
  }
})

// Watch for saved searches changes
watch(savedSearches, (newVal) => {
  localStorage.setItem('savedSearches', JSON.stringify(newVal))
}, { deep: true })
</script>

<style scoped>
.advanced-search {
  max-width: 1200px;
  margin: 0 auto;
  padding: 24px;
}

/* Search Header */
.search-header {
  display: flex;
  gap: 16px;
  margin-bottom: 24px;
  align-items: center;
}

.search-input-container {
  flex: 1;
}

.main-search-input {
  :deep(.el-input__wrapper) {
    padding-right: 0;
  }
  
  :deep(.el-input-group__append) {
    padding: 0;
    
    .el-button {
      border: none;
      height: 100%;
      margin: 0;
    }
  }
}

.search-actions {
  display: flex;
  gap: 8px;
  align-items: center;
  
  .el-button {
    &.active {
      color: var(--el-color-primary);
    }
  }
}

.filter-badge {
  margin-left: 4px;
}

/* Filters Panel */
.filters-panel {
  margin-bottom: 24px;
}

.filters-card {
  .filters-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  
  .filters-content {
    .custom-attributes {
      .attribute-item {
        display: flex;
        gap: 8px;
        margin-bottom: 8px;
        align-items: center;
      }
    }
  }
  
  .filters-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
}

/* Search Results */
.search-results {
  .results-summary {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 20px;
    padding-bottom: 16px;
    border-bottom: 1px solid var(--el-border-color-lighter);
    
    .summary-info {
      .results-count {
        font-weight: var(--font-weight-semibold);
        margin-right: 12px;
      }
      
      .search-time {
        color: var(--el-text-color-secondary);
        font-size: var(--font-size-sm);
      }
    }
    
    .results-actions {
      display: flex;
      gap: 12px;
      align-items: center;
    }
  }
}

/* Results List View */
.results-list {
  .empty-results {
    text-align: center;
    padding: 60px 20px;
    color: var(--el-text-color-secondary);
    
    .el-icon {
      font-size: 48px;
      margin-bottom: 16px;
      opacity: 0.5;
    }
  }
  
  .result-items {
    .result-item {
      display: flex;
      align-items: center;
      padding: 16px;
      border-bottom: 1px solid var(--el-border-color-lighter);
      cursor: pointer;
      transition: var(--transition-base);
      
      &:hover {
        background-color: var(--el-fill-color-lighter);
      }
      
      .item-icon {
        width: 48px;
        height: 48px;
        border-radius: var(--border-radius-md);
        display: flex;
        align-items: center;
        justify-content: center;
        margin-right: 16px;
        flex-shrink: 0;
        
        .el-icon {
          font-size: 24px;
        }
        
        &.services {
          background-color: var(--el-color-primary-light-9);
          color: var(--el-color-primary);
        }
        
        &.tools {
          background-color: var(--el-color-success-light-9);
          color: var(--el-color-success);
        }
        
        &.agents {
          background-color: var(--el-color-warning-light-9);
          color: var(--el-color-warning);
        }
      }
      
      .item-content {
        flex: 1;
        min-width: 0;
        
        .item-title {
          font-weight: var(--font-weight-medium);
          margin-bottom: 4px;
        }
        
        .item-description {
          color: var(--el-text-color-secondary);
          font-size: var(--font-size-sm);
          margin-bottom: 8px;
          @include text-truncate;
        }
        
        .item-meta {
          display: flex;
          align-items: center;
          gap: 8px;
          
          .item-date {
            color: var(--el-text-color-placeholder);
            font-size: var(--font-size-xs);
          }
        }
      }
      
      .item-actions {
        margin-left: 16px;
      }
    }
  }
}

/* Results Grid View */
.results-grid {
  .result-card {
    cursor: pointer;
    transition: var(--transition-base);
    
    &:hover {
      transform: translateY(-2px);
    }
    
    .card-icon {
      text-align: center;
      margin-bottom: 16px;
      
      &.services {
        color: var(--el-color-primary);
      }
      
      &.tools {
        color: var(--el-color-success);
      }
      
      &.agents {
        color: var(--el-color-warning);
      }
    }
    
    .card-content {
      h3 {
        margin: 0 0 8px 0;
        font-size: var(--font-size-lg);
      }
      
      p {
        color: var(--el-text-color-secondary);
        font-size: var(--font-size-sm);
        margin: 0 0 16px 0;
        @include text-truncate-2;
      }
      
      .card-footer {
        display: flex;
        justify-content: space-between;
        align-items: center;
        
        .card-date {
          color: var(--el-text-color-placeholder);
          font-size: var(--font-size-xs);
        }
      }
    }
  }
}

/* Pagination */
.results-pagination {
  margin-top: 24px;
  display: flex;
  justify-content: center;
}

/* Saved Searches */
.saved-searches {
  .empty-saved {
    text-align: center;
    padding: 40px;
    color: var(--el-text-color-secondary);
  }
  
  .saved-searches-list {
    .saved-search-item {
      display: flex;
      justify-content: space-between;
      align-items: center;
      padding: 12px;
      border-bottom: 1px solid var(--el-border-color-lighter);
      
      &:hover {
        background-color: var(--el-fill-color-lighter);
      }
      
      .saved-search-info {
        .saved-search-name {
          font-weight: var(--font-weight-medium);
          margin-bottom: 4px;
        }
        
        .saved-search-query {
          color: var(--el-text-color-secondary);
          font-size: var(--font-size-sm);
          margin-bottom: 4px;
        }
        
        .saved-search-meta {
          display: flex;
          align-items: center;
          gap: 8px;
          
          span {
            color: var(--el-text-color-placeholder);
            font-size: var(--font-size-xs);
          }
        }
      }
      
      .saved-search-actions {
        display: flex;
        gap: 8px;
      }
    }
  }
}

/* Responsive Design */
@media (max-width: 768px) {
  .advanced-search {
    padding: 16px;
  }
  
  .search-header {
    flex-direction: column;
    
    .search-input-container {
      width: 100%;
    }
    
    .search-actions {
      width: 100%;
      justify-content: space-between;
    }
  }
  
  .results-summary {
    flex-direction: column;
    align-items: flex-start !important;
    gap: 12px;
  }
  
  .result-item {
    flex-direction: column;
    align-items: flex-start !important;
    
    .item-icon {
      margin-right: 0 !important;
      margin-bottom: 12px;
    }
    
    .item-actions {
      margin-left: 0 !important;
      margin-top: 12px;
      width: 100%;
      justify-content: flex-end;
    }
  }
}
</style>