<template>
  <div class="workspace-manager">
    <!-- Header -->
    <div class="page-header">
      <div class="header-content">
        <div class="header-title">
          <h1>工作空间管理</h1>
          <p class="subtitle">管理和切换不同的数据工作空间</p>
        </div>
        <div class="header-actions">
          <el-button
            type="primary"
            :icon="Plus"
            @click="showCreateDialog = true"
          >
            创建工作空间
          </el-button>
          <el-button
            :icon="Refresh"
            @click="refreshWorkspaces"
            :loading="isLoading"
          >
            刷新
          </el-button>
        </div>
      </div>
    </div>

    <!-- Workspace Stats -->
    <div class="stats-section">
      <div class="stats-container">
        <div class="stat-card">
          <div class="stat-icon primary">
            <el-icon><FolderOpened /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ workspaceStats.total }}</div>
            <div class="stat-label">总工作空间</div>
          </div>
        </div>
        <div class="stat-card">
          <div class="stat-icon success">
            <el-icon><FolderOpened /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ workspaceStats.active }}</div>
            <div class="stat-label">当前活跃</div>
          </div>
        </div>
        <div class="stat-card">
          <div class="stat-icon info">
            <el-icon><Document /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ workspaceStats.totalServices }}</div>
            <div class="stat-label">服务总数</div>
          </div>
        </div>
        <div class="stat-card">
          <div class="stat-icon warning">
            <el-icon><Tools /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ workspaceStats.totalTools }}</div>
            <div class="stat-label">工具总数</div>
          </div>
        </div>
      </div>
    </div>

    <!-- Workspace List -->
    <div class="workspace-list-section">
      <div class="section-header">
        <h2>工作空间列表</h2>
        <div class="filter-controls">
          <el-input
            v-model="searchQuery"
            placeholder="搜索工作空间..."
            :prefix-icon="Search"
            clearable
            style="width: 300px"
          />
          <el-select v-model="sortBy" placeholder="排序方式" style="width: 150px">
            <el-option label="名称" value="name" />
            <el-option label="创建时间" value="created" />
            <el-option label="最后更新" value="updated" />
          </el-select>
        </div>
      </div>

      <div class="workspace-grid">
        <div
          v-for="workspace in filteredWorkspaces"
          :key="workspace.name"
          class="workspace-card"
          :class="{ active: workspace.is_current }"
        >
          <div class="card-header">
            <div class="workspace-info">
              <h3>{{ workspace.name }}</h3>
              <p class="path">{{ workspace.path }}</p>
            </div>
            <div class="workspace-status">
              <el-tag
                :type="workspace.is_current ? 'success' : 'info'"
                size="small"
              >
                {{ workspace.is_current ? '当前' : '可用' }}
              </el-tag>
            </div>
          </div>

          <div class="card-content">
            <div class="workspace-details">
              <div class="detail-item">
                <span class="label">服务数量</span>
                <span class="value">{{ workspace.service_count || 0 }}</span>
              </div>
              <div class="detail-item">
                <span class="label">创建时间</span>
                <span class="value">{{ formatDate(workspace.created_at) }}</span>
              </div>
              <div class="detail-item">
                <span class="label">最后更新</span>
                <span class="value">{{ formatDate(workspace.updated_at) }}</span>
              </div>
            </div>
          </div>

          <div class="card-actions">
            <el-button
              v-if="!workspace.is_current"
              type="primary"
              size="small"
              @click="switchWorkspace(workspace.name)"
            >
              切换
            </el-button>
            <el-button
              size="small"
              @click="editWorkspace(workspace)"
            >
              编辑
            </el-button>
            <el-button
              size="small"
              type="danger"
              @click="confirmDelete(workspace)"
              :disabled="workspace.is_current"
            >
              删除
            </el-button>
          </div>
        </div>

        <!-- Empty State -->
        <div v-if="filteredWorkspaces.length === 0" class="empty-state">
          <el-icon><Box /></el-icon>
          <h3>暂无工作空间</h3>
          <p>创建您的第一个工作空间来开始管理服务</p>
          <el-button type="primary" @click="showCreateDialog = true">
            创建工作空间
          </el-button>
        </div>
      </div>
    </div>

    <!-- Create/Edit Dialog -->
    <el-dialog
      v-model="showCreateDialog"
      :title="editingWorkspace ? '编辑工作空间' : '创建工作空间'"
      width="500px"
    >
      <el-form
        ref="formRef"
        :model="workspaceForm"
        :rules="workspaceRules"
        label-width="100px"
      >
        <el-form-item label="名称" prop="name">
          <el-input
            v-model="workspaceForm.name"
            placeholder="请输入工作空间名称"
            :disabled="!!editingWorkspace"
          />
        </el-form-item>
        <el-form-item label="路径" prop="path">
          <el-input
            v-model="workspaceForm.path"
            placeholder="请输入工作空间路径"
          >
            <template #append>
              <el-button @click="selectPath">选择</el-button>
            </template>
          </el-input>
        </el-form-item>
        <el-form-item label="描述" prop="description">
          <el-input
            v-model="workspaceForm.description"
            type="textarea"
            :rows="3"
            placeholder="工作空间描述（可选）"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showCreateDialog = false">取消</el-button>
        <el-button
          type="primary"
          @click="saveWorkspace"
          :loading="isSaving"
        >
          {{ editingWorkspace ? '保存' : '创建' }}
        </el-button>
      </template>
    </el-dialog>

    <!-- Delete Confirm Dialog -->
    <el-dialog
      v-model="showDeleteDialog"
      title="删除工作空间"
      width="400px"
    >
      <div class="delete-confirm">
        <el-icon class="warning-icon"><WarningFilled /></el-icon>
        <p>确定要删除工作空间 "{{ deletingWorkspace?.name }}" 吗？</p>
        <p class="warning-text">此操作不可撤销，所有相关数据将被永久删除。</p>
      </div>
      <template #footer>
        <el-button @click="showDeleteDialog = false">取消</el-button>
        <el-button
          type="danger"
          @click="deleteWorkspace"
          :loading="isDeleting"
        >
          删除
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  Plus, Refresh, Search, FolderOpened, Document, Tools,
  WarningFilled, Box
} from '@element-plus/icons-vue'
import { api } from '@/api'

// Reactive Data
const isLoading = ref(false)
const isSaving = ref(false)
const isDeleting = ref(false)
const showCreateDialog = ref(false)
const showDeleteDialog = ref(false)

const workspaces = ref([])
const searchQuery = ref('')
const sortBy = ref('name')
const editingWorkspace = ref(null)
const deletingWorkspace = ref(null)

const workspaceForm = ref({
  name: '',
  path: '',
  description: ''
})

const workspaceRules = {
  name: [
    { required: true, message: '请输入工作空间名称', trigger: 'blur' },
    { min: 2, max: 50, message: '长度在 2 到 50 个字符', trigger: 'blur' }
  ],
  path: [
    { required: true, message: '请输入工作空间路径', trigger: 'blur' }
  ]
}

const formRef = ref()

// Computed Properties
const workspaceStats = computed(() => ({
  total: workspaces.value.length,
  active: workspaces.value.filter(w => w.is_current).length,
  totalServices: workspaces.value.reduce((sum, w) => sum + (w.service_count || 0), 0),
  totalTools: workspaces.value.reduce((sum, w) => sum + (w.tool_count || 0), 0)
}))

const filteredWorkspaces = computed(() => {
  let filtered = [...workspaces.value]

  // Filter by search query
  if (searchQuery.value) {
    const query = searchQuery.value.toLowerCase()
    filtered = filtered.filter(w =>
      w.name.toLowerCase().includes(query) ||
      w.path.toLowerCase().includes(query) ||
      (w.description && w.description.toLowerCase().includes(query))
    )
  }

  // Sort
  filtered.sort((a, b) => {
    switch (sortBy.value) {
      case 'name':
        return a.name.localeCompare(b.name)
      case 'created':
        return new Date(b.created_at) - new Date(a.created_at)
      case 'updated':
        return new Date(b.updated_at) - new Date(a.updated_at)
      default:
        return 0
    }
  })

  return filtered
})

// Methods
const refreshWorkspaces = async () => {
  isLoading.value = true
  try {
    const response = await api.dataSpace.getWorkspaceList()
    if (response.data?.success) {
      workspaces.value = response.data.data.workspaces || []
      
      // Get current workspace
      try {
        const currentResponse = await api.dataSpace.getCurrentWorkspace()
        if (currentResponse.data?.success) {
          const currentName = currentResponse.data.data.workspace?.name
          workspaces.value.forEach(w => {
            w.is_current = w.name === currentName
          })
        }
      } catch (error) {
        console.warn('Failed to get current workspace:', error)
      }
    }
  } catch (error) {
    ElMessage.error('获取工作空间列表失败')
    console.error('Failed to fetch workspaces:', error)
  } finally {
    isLoading.value = false
  }
}

const switchWorkspace = async (name) => {
  try {
    await ElMessageBox.confirm(
      `确定要切换到工作空间 "${name}" 吗？`,
      '切换工作空间',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )

    const response = await api.dataSpace.switchWorkspace(name)
    if (response.data?.success) {
      ElMessage.success(`已切换到工作空间 "${name}"`)
      await refreshWorkspaces()
      
      // Refresh other data
      window.location.reload()
    }
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error('切换工作空间失败')
      console.error('Failed to switch workspace:', error)
    }
  }
}

const editWorkspace = (workspace) => {
  editingWorkspace.value = workspace
  workspaceForm.value = {
    name: workspace.name,
    path: workspace.path,
    description: workspace.description || ''
  }
  showCreateDialog.value = true
}

const confirmDelete = (workspace) => {
  deletingWorkspace.value = workspace
  showDeleteDialog.value = true
}

const deleteWorkspace = async () => {
  if (!deletingWorkspace.value) return

  isDeleting.value = true
  try {
    const response = await api.dataSpace.deleteWorkspace(deletingWorkspace.value.name)
    if (response.data?.success) {
      ElMessage.success(`工作空间 "${deletingWorkspace.value.name}" 已删除`)
      showDeleteDialog.value = false
      await refreshWorkspaces()
    }
  } catch (error) {
    ElMessage.error('删除工作空间失败')
    console.error('Failed to delete workspace:', error)
  } finally {
    isDeleting.value = false
    deletingWorkspace.value = null
  }
}

const saveWorkspace = async () => {
  if (!formRef.value) return

  try {
    await formRef.value.validate()
  } catch (error) {
    return
  }

  isSaving.value = true
  try {
    if (editingWorkspace.value) {
      // Update workspace
      const response = await api.dataSpace.updateWorkspace(
        editingWorkspace.value.name,
        workspaceForm.value
      )
      if (response.data?.success) {
        ElMessage.success('工作空间已更新')
        showCreateDialog.value = false
        await refreshWorkspaces()
      }
    } else {
      // Create new workspace
      const response = await api.dataSpace.createWorkspace(workspaceForm.value)
      if (response.data?.success) {
        ElMessage.success(`工作空间 "${workspaceForm.value.name}" 已创建`)
        showCreateDialog.value = false
        await refreshWorkspaces()
      }
    }
  } catch (error) {
    ElMessage.error(editingWorkspace.value ? '更新工作空间失败' : '创建工作空间失败')
    console.error('Failed to save workspace:', error)
  } finally {
    isSaving.value = false
  }
}

const selectPath = () => {
  // In a real implementation, this would open a directory picker
  ElMessage.info('请手动输入工作空间路径')
}

const formatDate = (dateString) => {
  if (!dateString) return '未知'
  
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
  await refreshWorkspaces()
})
</script>

<style scoped>
.workspace-manager {
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

.stats-container {
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

.stat-icon.info {
  background: var(--info-lighter);
  color: var(--info-color);
}

.stat-icon.warning {
  background: var(--warning-lighter);
  color: var(--warning-color);
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

/* Workspace List Section */
.workspace-list-section {
  margin-bottom: 32px;
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 24px;
}

.section-header h2 {
  font-size: var(--font-size-2xl);
  font-weight: var(--font-weight-semibold);
  margin: 0;
  color: var(--text-primary);
}

.filter-controls {
  display: flex;
  gap: 12px;
}

.workspace-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(350px, 1fr));
  gap: 20px;
}

.workspace-card {
  background: var(--bg-color);
  border-radius: var(--border-radius-lg);
  border: 2px solid transparent;
  box-shadow: var(--shadow-base);
  transition: var(--transition-base);
  overflow: hidden;
}

.workspace-card:hover {
  box-shadow: var(--shadow-md);
  transform: translateY(-2px);
}

.workspace-card.active {
  border-color: var(--primary-color);
}

.card-header {
  padding: 20px;
  border-bottom: 1px solid var(--border-lighter);
  display: flex;
  justify-content: space-between;
  align-items: start;
}

.workspace-info h3 {
  font-size: var(--font-size-lg);
  font-weight: var(--font-weight-semibold);
  margin: 0 0 4px 0;
  color: var(--text-primary);
}

.workspace-info .path {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
  margin: 0;
  font-family: var(--font-family-mono);
}

.card-content {
  padding: 20px;
}

.workspace-details {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.detail-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.detail-item .label {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
}

.detail-item .value {
  font-size: var(--font-size-sm);
  font-weight: var(--font-weight-medium);
  color: var(--text-primary);
}

.card-actions {
  padding: 16px 20px;
  background: var(--bg-color-secondary);
  display: flex;
  gap: 8px;
  justify-content: flex-end;
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

/* Dialog Styles */
.delete-confirm {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  padding: 20px 0;
}

.warning-icon {
  font-size: 48px;
  color: var(--warning-color);
  margin-bottom: 16px;
}

.delete-confirm p {
  margin: 0 0 8px 0;
  color: var(--text-primary);
}

.warning-text {
  color: var(--text-secondary) !important;
  font-size: var(--font-size-sm) !important;
}

/* Responsive Design */
@media (max-width: 768px) {
  .header-content {
    flex-direction: column;
    gap: 16px;
    align-items: flex-start;
  }

  .section-header {
    flex-direction: column;
    gap: 16px;
    align-items: flex-start;
  }

  .filter-controls {
    width: 100%;
  }

  .filter-controls .el-input,
  .filter-controls .el-select {
    width: 100% !important;
  }

  .workspace-grid {
    grid-template-columns: 1fr;
  }

  .stats-container {
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  }
}
</style>