<template>
  <div class="tool-templates">
    <!-- Page Header -->
    <div class="page-header">
      <div class="header-content">
        <div class="header-title">
          <h1>工具模板库</h1>
          <p class="subtitle">管理和使用工具执行模板，提高工作效率</p>
        </div>
        <div class="header-actions">
          <el-button
            type="primary"
            :icon="Plus"
            @click="showCreateDialog = true"
          >
            创建模板
          </el-button>
          <el-button
            :icon="Refresh"
            @click="refreshTemplates"
            :loading="loading"
          >
            刷新
          </el-button>
        </div>
      </div>
    </div>

    <!-- Stats Overview -->
    <div class="stats-section">
      <div class="stats-grid">
        <div class="stat-card">
          <div class="stat-icon primary">
            <el-icon><Document /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ templates.length }}</div>
            <div class="stat-label">总模板数</div>
          </div>
        </div>
        <div class="stat-card">
          <div class="stat-icon success">
            <el-icon><Star /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ favoriteTemplates.length }}</div>
            <div class="stat-label">收藏模板</div>
          </div>
        </div>
        <div class="stat-card">
          <div class="stat-icon warning">
            <el-icon><Clock /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ recentTemplates.length }}</div>
            <div class="stat-label">最近使用</div>
          </div>
        </div>
        <div class="stat-card">
          <div class="stat-icon info">
            <el-icon><Tools /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ uniqueToolsCount }}</div>
            <div class="stat-label">覆盖工具数</div>
          </div>
        </div>
      </div>
    </div>

    <!-- Filter and Search -->
    <el-card class="filter-card">
      <el-row :gutter="20">
        <el-col :xs="24" :sm="12" :md="8">
          <el-input
            v-model="searchQuery"
            placeholder="搜索模板名称、描述..."
            :prefix-icon="Search"
            clearable
            @input="handleSearch"
          />
        </el-col>
        <el-col :xs="24" :sm="12" :md="6">
          <el-select
            v-model="categoryFilter"
            placeholder="按分类筛选"
            clearable
            @change="handleFilter"
          >
            <el-option label="全部分类" value="" />
            <el-option label="数据处理" value="data-processing" />
            <el-option label="文件操作" value="file-ops" />
            <el-option label="网络请求" value="network" />
            <el-option label="文本处理" value="text" />
            <el-option label="开发工具" value="dev-tools" />
            <el-option label="系统管理" value="system" />
            <el-option label="AI/ML" value="ai-ml" />
            <el-option label="其他" value="other" />
          </el-select>
        </el-col>
        <el-col :xs="24" :sm="12" :md="6">
          <el-select
            v-model="toolFilter"
            placeholder="按工具筛选"
            clearable
            filterable
            @change="handleFilter"
          >
            <el-option label="全部工具" value="" />
            <el-option
              v-for="tool in availableTools"
              :key="tool"
              :label="tool"
              :value="tool"
            />
          </el-select>
        </el-col>
        <el-col :xs="24" :sm="12" :md="4">
          <el-select v-model="sortBy" placeholder="排序方式" @change="handleSort">
            <el-option label="名称" value="name" />
            <el-option label="创建时间" value="created" />
            <el-option label="使用次数" value="usage" />
            <el-option label="收藏优先" value="favorites" />
          </el-select>
        </el-col>
      </el-row>
    </el-card>

    <!-- Templates Grid -->
    <div class="templates-grid">
      <div
        v-for="template in filteredTemplates"
        :key="template.id"
        class="template-card"
        :class="{ 'is-favorite': template.isFavorite }"
      >
        <div class="template-header">
          <div class="template-info">
            <div class="template-name">
              <h3>{{ template.name }}</h3>
              <el-tag size="small" :type="getCategoryType(template.category)">
                {{ getCategoryLabel(template.category) }}
              </el-tag>
            </div>
            <div class="template-meta">
              <span class="tool-name">{{ template.toolName }}</span>
              <span class="usage-count">使用 {{ template.usageCount || 0 }} 次</span>
            </div>
          </div>
          <div class="template-actions">
            <el-button
              text
              :icon="template.isFavorite ? StarFilled : Star"
              @click="toggleFavorite(template)"
              :class="{ 'is-favorite': template.isFavorite }"
            />
          </div>
        </div>

        <div class="template-body">
          <p class="template-description">{{ template.description }}</p>
          
          <!-- Parameters Preview -->
          <div v-if="template.parameters && Object.keys(template.parameters).length > 0" class="params-preview">
            <div class="params-label">参数预览:</div>
            <div class="params-list">
              <el-tag
                v-for="(value, key) in template.parameters"
                :key="key"
                size="small"
                class="param-tag"
              >
                {{ key }}: {{ typeof value === 'object' ? JSON.stringify(value) : String(value) }}
              </el-tag>
            </div>
          </div>

          <!-- Tags -->
          <div v-if="template.tags && template.tags.length > 0" class="template-tags">
            <el-tag
              v-for="tag in template.tags"
              :key="tag"
              size="small"
              effect="plain"
              class="tag-item"
            >
              {{ tag }}
            </el-tag>
          </div>
        </div>

        <div class="template-footer">
          <div class="template-stats">
            <span class="created-time">创建于 {{ formatDate(template.createdAt) }}</span>
            <span v-if="template.lastUsed" class="last-used">
              最近使用: {{ formatDate(template.lastUsed) }}
            </span>
          </div>
          <div class="template-actions">
            <el-button
              size="small"
              @click="previewTemplate(template)"
            >
              预览
            </el-button>
            <el-button
              size="small"
              type="primary"
              @click="useTemplate(template)"
            >
              使用模板
            </el-button>
            <el-dropdown @command="(cmd) => handleTemplateAction(cmd, template)">
              <el-button size="small" text>
                <el-icon><MoreFilled /></el-icon>
              </el-button>
              <template #dropdown>
                <el-dropdown-menu>
                  <el-dropdown-item command="edit" :icon="Edit">编辑</el-dropdown-item>
                  <el-dropdown-item command="duplicate" :icon="CopyDocument">复制</el-dropdown-item>
                  <el-dropdown-item command="export" :icon="Download">导出</el-dropdown-item>
                  <el-dropdown-item command="delete" divided :icon="Delete">删除</el-dropdown-item>
                </el-dropdown-menu>
              </template>
            </el-dropdown>
          </div>
        </div>
      </div>

      <!-- Empty State -->
      <div v-if="filteredTemplates.length === 0 && !loading" class="empty-state">
        <el-icon class="empty-icon"><Document /></el-icon>
        <h3>暂无模板</h3>
        <p>{{ searchQuery || categoryFilter || toolFilter ? '没有找到匹配的模板' : '创建您的第一个模板来开始' }}</p>
        <el-button v-if="!searchQuery && !categoryFilter && !toolFilter" type="primary" @click="showCreateDialog = true">
          创建模板
        </el-button>
      </div>
    </div>

    <!-- Create/Edit Template Dialog -->
    <el-dialog
      v-model="showCreateDialog"
      :title="editingTemplate ? '编辑模板' : '创建新模板'"
      width="800px"
      :close-on-click-modal="false"
    >
      <el-form
        ref="formRef"
        :model="templateForm"
        :rules="templateRules"
        label-width="100px"
      >
        <el-form-item label="模板名称" prop="name">
          <el-input
            v-model="templateForm.name"
            placeholder="输入模板名称"
            maxlength="50"
            show-word-limit
          />
        </el-form-item>

        <el-form-item label="选择工具" prop="toolName">
          <el-select
            v-model="templateForm.toolName"
            placeholder="选择要创建模板的工具"
            filterable
            @change="onToolChange"
          >
            <el-option
              v-for="tool in systemStore.tools"
              :key="tool.name"
              :label="tool.name"
              :value="tool.name"
            >
              <div class="tool-option">
                <span class="tool-name">{{ tool.name }}</span>
                <span class="tool-desc">{{ tool.description || '无描述' }}</span>
              </div>
            </el-option>
          </el-select>
        </el-form-item>

        <el-form-item label="描述" prop="description">
          <el-input
            v-model="templateForm.description"
            type="textarea"
            :rows="3"
            placeholder="描述模板的用途和特点"
            maxlength="200"
            show-word-limit
          />
        </el-form-item>

        <el-form-item label="分类" prop="category">
          <el-select v-model="templateForm.category" placeholder="选择模板分类">
            <el-option label="数据处理" value="data-processing" />
            <el-option label="文件操作" value="file-ops" />
            <el-option label="网络请求" value="network" />
            <el-option label="文本处理" value="text" />
            <el-option label="开发工具" value="dev-tools" />
            <el-option label="系统管理" value="system" />
            <el-option label="AI/ML" value="ai-ml" />
            <el-option label="其他" value="other" />
          </el-select>
        </el-form-item>

        <el-form-item label="标签">
          <el-select
            v-model="templateForm.tags"
            multiple
            filterable
            allow-create
            default-first-option
            placeholder="添加标签"
          >
            <el-option
              v-for="tag in availableTags"
              :key="tag"
              :label="tag"
              :value="tag"
            />
          </el-select>
        </el-form-item>

        <!-- Parameters Configuration -->
        <el-form-item v-if="selectedTool" label="参数配置">
          <div class="params-config">
            <div class="params-help">
              配置工具参数的默认值。参数值将作为模板保存。
            </div>
            <template v-if="toolParameters && Object.keys(toolParameters).length > 0">
              <div
                v-for="(param, key) in toolParameters"
                :key="key"
                class="param-item"
              >
                <div class="param-info">
                  <div class="param-name">{{ key }}</div>
                  <div class="param-desc">{{ param.description || '无描述' }}</div>
                  <el-tag size="small" :type="param.required ? 'danger' : 'info'">
                    {{ param.required ? '必需' : '可选' }}
                  </el-tag>
                </div>
                <div class="param-input">
                  <!-- String input -->
                  <el-input
                    v-if="param.type === 'string'"
                    v-model="templateForm.parameters[key]"
                    :placeholder="param.default || '输入值'"
                  />
                  <!-- Number input -->
                  <el-input-number
                    v-else-if="param.type === 'number'"
                    v-model="templateForm.parameters[key]"
                    :placeholder="param.default"
                  />
                  <!-- Boolean input -->
                  <el-switch
                    v-else-if="param.type === 'boolean'"
                    v-model="templateForm.parameters[key]"
                  />
                  <!-- Array input -->
                  <el-select
                    v-else-if="param.type === 'array'"
                    v-model="templateForm.parameters[key]"
                    multiple
                    filterable
                    allow-create
                    default-first-option
                    placeholder="输入多个值"
                  />
                  <!-- Object input -->
                  <el-input
                    v-else-if="param.type === 'object'"
                    v-model="templateForm.parametersJson[key]"
                    type="textarea"
                    :rows="2"
                    placeholder="输入JSON格式的值"
                  />
                  <!-- Default fallback -->
                  <el-input
                    v-else
                    v-model="templateForm.parameters[key]"
                    :placeholder="param.default || '输入值'"
                  />
                </div>
              </div>
            </template>
            <div v-else class="no-params">
              该工具没有参数
            </div>
          </div>
        </el-form-item>
      </el-form>

      <template #footer>
        <el-button @click="showCreateDialog = false">取消</el-button>
        <el-button type="primary" @click="saveTemplate" :loading="saving">
          {{ editingTemplate ? '更新' : '创建' }}
        </el-button>
      </template>
    </el-dialog>

    <!-- Preview Template Dialog -->
    <el-dialog
      v-model="showPreviewDialog"
      title="模板预览"
      width="700px"
    >
      <div v-if="previewingTemplate" class="template-preview">
        <el-descriptions :column="1" border>
          <el-descriptions-item label="模板名称">
            {{ previewingTemplate.name }}
          </el-descriptions-item>
          <el-descriptions-item label="工具名称">
            {{ previewingTemplate.toolName }}
          </el-descriptions-item>
          <el-descriptions-item label="描述">
            {{ previewingTemplate.description }}
          </el-descriptions-item>
          <el-descriptions-item label="分类">
            {{ getCategoryLabel(previewingTemplate.category) }}
          </el-descriptions-item>
          <el-descriptions-item label="标签">
            <div v-if="previewingTemplate.tags && previewingTemplate.tags.length > 0">
              <el-tag
                v-for="tag in previewingTemplate.tags"
                :key="tag"
                size="small"
                class="mr-1"
              >
                {{ tag }}
              </el-tag>
            </div>
            <span v-else>-</span>
          </el-descriptions-item>
        </el-descriptions>

        <!-- Parameters Details -->
        <div v-if="previewingTemplate.parameters && Object.keys(previewingTemplate.parameters).length > 0" class="params-section">
          <h4>参数配置</h4>
          <el-descriptions :column="1" border>
            <el-descriptions-item
              v-for="(value, key) in previewingTemplate.parameters"
              :key="key"
              :label="key"
            >
              <code>{{ typeof value === 'object' ? JSON.stringify(value, null, 2) : String(value) }}</code>
            </el-descriptions-item>
          </el-descriptions>
        </div>
      </div>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useSystemStore } from '@/stores/system'
import {
  Plus, Refresh, Search, Document, Star, StarFilled,
  Clock, Tools, Edit, CopyDocument, Download, Delete,
  MoreFilled
} from '@element-plus/icons-vue'

const router = useRouter()
const systemStore = useSystemStore()

// Reactive Data
const loading = ref(false)
const saving = ref(false)
const searchQuery = ref('')
const categoryFilter = ref('')
const toolFilter = ref('')
const sortBy = ref('created')
const showCreateDialog = ref(false)
const showPreviewDialog = ref(false)
const editingTemplate = ref(null)
const previewingTemplate = ref(null)
const selectedTool = ref(null)
const toolParameters = ref({})

const templateForm = ref({
  name: '',
  toolName: '',
  description: '',
  category: 'other',
  tags: [],
  parameters: {},
  parametersJson: {}
})

const templateRules = {
  name: [
    { required: true, message: '请输入模板名称', trigger: 'blur' },
    { min: 2, max: 50, message: '长度在 2 到 50 个字符', trigger: 'blur' }
  ],
  toolName: [
    { required: true, message: '请选择工具', trigger: 'change' }
  ],
  description: [
    { required: true, message: '请输入描述', trigger: 'blur' }
  ],
  category: [
    { required: true, message: '请选择分类', trigger: 'change' }
  ]
}

const formRef = ref()

// Templates data
const templates = ref([])

// Computed Properties
const availableTools = computed(() => {
  return Array.from(new Set(systemStore.tools.map(tool => tool.name))).sort()
})

const availableTags = computed(() => {
  const tags = new Set()
  templates.value.forEach(template => {
    if (template.tags) {
      template.tags.forEach(tag => tags.add(tag))
    }
  })
  return Array.from(tags).sort()
})

const filteredTemplates = computed(() => {
  let filtered = [...templates.value]

  // Search filter
  if (searchQuery.value) {
    const query = searchQuery.value.toLowerCase()
    filtered = filtered.filter(template =>
      template.name.toLowerCase().includes(query) ||
      template.description.toLowerCase().includes(query) ||
      (template.tags && template.tags.some(tag => tag.toLowerCase().includes(query)))
    )
  }

  // Category filter
  if (categoryFilter.value) {
    filtered = filtered.filter(template => template.category === categoryFilter.value)
  }

  // Tool filter
  if (toolFilter.value) {
    filtered = filtered.filter(template => template.toolName === toolFilter.value)
  }

  // Sort
  filtered.sort((a, b) => {
    switch (sortBy.value) {
      case 'name':
        return a.name.localeCompare(b.name)
      case 'created':
        return new Date(b.createdAt) - new Date(a.createdAt)
      case 'usage':
        return (b.usageCount || 0) - (a.usageCount || 0)
      case 'favorites':
        return (b.isFavorite ? 1 : 0) - (a.isFavorite ? 1 : 0)
      default:
        return 0
    }
  })

  return filtered
})

const favoriteTemplates = computed(() => {
  return templates.value.filter(t => t.isFavorite)
})

const recentTemplates = computed(() => {
  return templates.value
    .filter(t => t.lastUsed)
    .sort((a, b) => new Date(b.lastUsed) - new Date(a.lastUsed))
    .slice(0, 5)
})

const uniqueToolsCount = computed(() => {
  return new Set(templates.value.map(t => t.toolName)).size
})

// Methods
const refreshTemplates = async () => {
  loading.value = true
  try {
    // Load templates from localStorage
    const saved = localStorage.getItem('toolTemplates')
    if (saved) {
      templates.value = JSON.parse(saved)
    }
  } catch (error) {
    ElMessage.error('加载模板失败')
    console.error('Failed to load templates:', error)
  } finally {
    loading.value = false
  }
}

const handleSearch = () => {
  // Filter is handled by computed property
}

const handleFilter = () => {
  // Filter is handled by computed property
}

const handleSort = () => {
  // Sort is handled by computed property
}

const onToolChange = (toolName) => {
  selectedTool.value = systemStore.tools.find(tool => tool.name === toolName)
  
  // Parse tool parameters
  if (selectedTool.value?.inputSchema?.properties) {
    toolParameters.value = {}
    for (const [key, param] of Object.entries(selectedTool.value.inputSchema.properties)) {
      toolParameters.value[key] = {
        type: param.type || 'string',
        description: param.description,
        required: selectedTool.value.inputSchema.required?.includes(key),
        default: param.default
      }
    }
  } else {
    toolParameters.value = {}
  }
  
  // Reset parameters
  templateForm.value.parameters = {}
  templateForm.value.parametersJson = {}
}

const saveTemplate = async () => {
  if (!formRef.value) return

  try {
    await formRef.value.validate()
  } catch (error) {
    return
  }

  saving.value = true
  try {
    // Process JSON parameters
    const parameters = { ...templateForm.value.parameters }
    for (const [key, value] of Object.entries(templateForm.value.parametersJson)) {
      if (value) {
        try {
          parameters[key] = JSON.parse(value)
        } catch (e) {
          parameters[key] = value
        }
      }
    }

    const template = {
      id: editingTemplate.value ? editingTemplate.value.id : Date.now(),
      name: templateForm.value.name,
      toolName: templateForm.value.toolName,
      description: templateForm.value.description,
      category: templateForm.value.category,
      tags: templateForm.value.tags || [],
      parameters,
      isFavorite: editingTemplate.value ? editingTemplate.value.isFavorite : false,
      usageCount: editingTemplate.value ? editingTemplate.value.usageCount : 0,
      createdAt: editingTemplate.value ? editingTemplate.value.createdAt : new Date().toISOString(),
      updatedAt: new Date().toISOString()
    }

    if (editingTemplate.value) {
      // Update existing template
      const index = templates.value.findIndex(t => t.id === editingTemplate.value.id)
      if (index > -1) {
        templates.value[index] = template
      }
      ElMessage.success('模板更新成功')
    } else {
      // Create new template
      templates.value.push(template)
      ElMessage.success('模板创建成功')
    }

    // Save to localStorage
    localStorage.setItem('toolTemplates', JSON.stringify(templates.value))
    
    showCreateDialog.value = false
    resetForm()
  } catch (error) {
    ElMessage.error('保存失败')
    console.error('Failed to save template:', error)
  } finally {
    saving.value = false
  }
}

const resetForm = () => {
  templateForm.value = {
    name: '',
    toolName: '',
    description: '',
    category: 'other',
    tags: [],
    parameters: {},
    parametersJson: {}
  }
  selectedTool.value = null
  toolParameters.value = {}
  editingTemplate.value = null
}

const previewTemplate = (template) => {
  previewingTemplate.value = template
  showPreviewDialog.value = true
}

const useTemplate = (template) => {
  // Update usage count
  template.usageCount = (template.usageCount || 0) + 1
  template.lastUsed = new Date().toISOString()
  localStorage.setItem('toolTemplates', JSON.stringify(templates.value))
  
  // Navigate to tool execution with pre-filled parameters
  router.push({
    path: '/tools/execute',
    query: {
      tool: template.toolName,
      template: template.id
    }
  })
}

const toggleFavorite = (template) => {
  template.isFavorite = !template.isFavorite
  localStorage.setItem('toolTemplates', JSON.stringify(templates.value))
}

const handleTemplateAction = async (command, template) => {
  switch (command) {
    case 'edit':
      editingTemplate.value = template
      templateForm.value = {
        name: template.name,
        toolName: template.toolName,
        description: template.description,
        category: template.category,
        tags: template.tags || [],
        parameters: { ...template.parameters },
        parametersJson: {}
      }
      onToolChange(template.toolName)
      showCreateDialog.value = true
      break
      
    case 'duplicate':
      const newTemplate = {
        ...template,
        id: Date.now(),
        name: `${template.name} (副本)`,
        usageCount: 0,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString()
      }
      delete newTemplate.lastUsed
      templates.value.push(newTemplate)
      localStorage.setItem('toolTemplates', JSON.stringify(templates.value))
      ElMessage.success('模板复制成功')
      break
      
    case 'export':
      const dataStr = JSON.stringify(template, null, 2)
      const dataBlob = new Blob([dataStr], { type: 'application/json' })
      const url = URL.createObjectURL(dataBlob)
      const link = document.createElement('a')
      link.href = url
      link.download = `${template.name}.json`
      link.click()
      URL.revokeObjectURL(url)
      ElMessage.success('模板导出成功')
      break
      
    case 'delete':
      try {
        await ElMessageBox.confirm(
          `确定要删除模板 "${template.name}" 吗？`,
          '删除确认',
          {
            confirmButtonText: '确定',
            cancelButtonText: '取消',
            type: 'warning'
          }
        )
        
        const index = templates.value.findIndex(t => t.id === template.id)
        if (index > -1) {
          templates.value.splice(index, 1)
          localStorage.setItem('toolTemplates', JSON.stringify(templates.value))
          ElMessage.success('删除成功')
        }
      } catch (error) {
        if (error !== 'cancel') {
          console.error('Delete failed:', error)
        }
      }
      break
  }
}

const getCategoryType = (category) => {
  const types = {
    'data-processing': 'primary',
    'file-ops': 'success',
    'network': 'warning',
    'text': 'info',
    'dev-tools': 'danger',
    'system': '',
    'ai-ml': 'primary',
    'other': 'info'
  }
  return types[category] || 'info'
}

const getCategoryLabel = (category) => {
  const labels = {
    'data-processing': '数据处理',
    'file-ops': '文件操作',
    'network': '网络请求',
    'text': '文本处理',
    'dev-tools': '开发工具',
    'system': '系统管理',
    'ai-ml': 'AI/ML',
    'other': '其他'
  }
  return labels[category] || '其他'
}

const formatDate = (dateString) => {
  if (!dateString) return '-'
  
  const date = new Date(dateString)
  const now = new Date()
  const diff = now - date
  
  if (diff < 60000) return '刚刚'
  if (diff < 3600000) return `${Math.floor(diff / 60000)} 分钟前`
  if (diff < 86400000) return `${Math.floor(diff / 3600000)} 小时前`
  if (diff < 604800000) return `${Math.floor(diff / 86400000)} 天前`
  
  return date.toLocaleDateString('zh-CN')
}

// Lifecycle
onMounted(async () => {
  await refreshTemplates()
  
  // Load tools if not already loaded
  if (systemStore.tools.length === 0) {
    await systemStore.fetchTools()
  }
})
</script>

<style lang="scss" scoped>
.tool-templates {
  max-width: 1400px;
  margin: 0 auto;
}

/* Page Header */
.page-header {
  margin-bottom: 32px;
}

.header-content {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.header-title h1 {
  font-size: var(--font-size-4xl);
  font-weight: var(--font-weight-bold);
  margin: 0 0 8px 0;
  color: var(--text-primary);
}

.header-title .subtitle {
  font-size: var(--font-size-lg);
  color: var(--text-secondary);
  margin: 0;
}

.header-actions {
  display: flex;
  gap: 12px;
}

/* Stats Section */
.stats-section {
  margin-bottom: 32px;
}

.stats-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
  gap: 20px;
}

.stat-card {
  background: var(--bg-color);
  border-radius: var(--border-radius-lg);
  padding: 24px;
  display: flex;
  align-items: center;
  gap: 16px;
  box-shadow: var(--shadow-base);
  transition: var(--transition-base);
}

.stat-card:hover {
  box-shadow: var(--shadow-md);
  transform: translateY(-2px);
}

.stat-icon {
  width: 56px;
  height: 56px;
  border-radius: var(--border-radius-xl);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 24px;
  flex-shrink: 0;
}

.stat-icon.primary {
  background: var(--primary-lighter);
  color: var(--primary-color);
}

.stat-icon.success {
  background: var(--success-lighter);
  color: var(--success-color);
}

.stat-icon.warning {
  background: var(--warning-lighter);
  color: var(--warning-color);
}

.stat-icon.info {
  background: var(--info-lighter);
  color: var(--info-color);
}

.stat-content .stat-value {
  font-size: var(--font-size-2xl);
  font-weight: var(--font-weight-bold);
  color: var(--text-primary);
  line-height: 1.2;
}

.stat-content .stat-label {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
  margin-top: 4px;
}

/* Filter Card */
.filter-card {
  margin-bottom: 24px;
}

/* Templates Grid */
.templates-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(380px, 1fr));
  gap: 24px;
}

.template-card {
  background: var(--bg-color);
  border-radius: var(--border-radius-lg);
  border: 2px solid transparent;
  box-shadow: var(--shadow-base);
  transition: var(--transition-base);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.template-card:hover {
  box-shadow: var(--shadow-md);
  transform: translateY(-2px);
}

.template-card.is-favorite {
  border-color: var(--warning-color);
}

.template-header {
  padding: 20px;
  border-bottom: 1px solid var(--border-lighter);
  display: flex;
  justify-content: space-between;
  align-items: start;
}

.template-info {
  flex: 1;
}

.template-name {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.template-name h3 {
  font-size: var(--font-size-lg);
  font-weight: var(--font-weight-semibold);
  margin: 0;
  color: var(--text-primary);
}

.template-meta {
  display: flex;
  gap: 16px;
  font-size: var(--font-size-xs);
  color: var(--text-secondary);
}

.tool-name {
  font-weight: var(--font-weight-medium);
  color: var(--primary-color);
}

.template-actions {
  display: flex;
  align-items: center;
}

.template-actions .el-button {
  color: var(--text-secondary);
  transition: var(--transition-fast);
}

.template-actions .el-button.is-favorite {
  color: var(--warning-color);
}

.template-body {
  padding: 20px;
  flex: 1;
}

.template-description {
  color: var(--text-regular);
  margin-bottom: 16px;
  line-height: 1.5;
}

.params-preview {
  margin-bottom: 16px;
}

.params-label {
  font-size: var(--font-size-xs);
  color: var(--text-secondary);
  margin-bottom: 8px;
  font-weight: var(--font-weight-medium);
}

.params-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.param-tag {
  font-family: var(--font-family-mono);
  font-size: var(--font-size-xs);
}

.template-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.tag-item {
  font-size: var(--font-size-xs);
}

.template-footer {
  padding: 16px 20px;
  background: var(--bg-color-secondary);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.template-stats {
  font-size: var(--font-size-xs);
  color: var(--text-secondary);
}

.template-stats .created-time,
.template-stats .last-used {
  display: block;
}

.template-footer .template-actions {
  display: flex;
  gap: 8px;
}

/* Empty State */
.empty-state {
  grid-column: 1 / -1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 80px 20px;
  text-align: center;
}

.empty-state .el-icon {
  font-size: 64px;
  color: var(--text-placeholder);
  margin-bottom: 16px;
}

.empty-state h3 {
  font-size: var(--font-size-xl);
  font-weight: var(--font-weight-semibold);
  margin: 0 0 8px 0;
  color: var(--text-primary);
}

.empty-state p {
  font-size: var(--font-size-base);
  color: var(--text-secondary);
  margin: 0 0 24px 0;
}

/* Form Styles */
.params-config {
  border: 1px solid var(--border-lighter);
  border-radius: var(--border-radius-md);
  padding: 16px;
}

.params-help {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
  margin-bottom: 16px;
  padding: 8px 12px;
  background: var(--info-lighter);
  border-radius: var(--border-radius-sm);
}

.param-item {
  display: flex;
  gap: 16px;
  margin-bottom: 16px;
  align-items: flex-start;
}

.param-item:last-child {
  margin-bottom: 0;
}

.param-info {
  flex: 0 0 200px;
}

.param-name {
  font-weight: var(--font-weight-medium);
  margin-bottom: 4px;
}

.param-desc {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
}

.param-input {
  flex: 1;
}

.no-params {
  text-align: center;
  color: var(--text-secondary);
  padding: 20px;
}

.tool-option {
  display: flex;
  flex-direction: column;
}

.tool-option .tool-name {
  font-weight: var(--font-weight-medium);
}

.tool-option .tool-desc {
  font-size: var(--font-size-xs);
  color: var(--text-secondary);
  margin-top: 2px;
}

/* Preview Styles */
.template-preview {
  .params-section {
    margin-top: 24px;
    
    h4 {
      margin-bottom: 16px;
      color: var(--text-primary);
    }
  }
}

/* Responsive Design */
@media (max-width: 768px) {
  .header-content {
    flex-direction: column;
    gap: 16px;
    align-items: flex-start;
  }

  .stats-grid {
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  }

  .templates-grid {
    grid-template-columns: 1fr;
  }

  .template-footer {
    flex-direction: column;
    gap: 12px;
    align-items: flex-start;
  }

  .param-item {
    flex-direction: column;
    gap: 8px;
  }

  .param-info {
    flex: 1;
  }
}
</style>