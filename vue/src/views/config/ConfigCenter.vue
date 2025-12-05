<template>
  <div class="config-center">
    <!-- Page Header -->
    <div class="page-header">
      <div class="header-content">
        <div class="header-title">
          <h1>配置中心</h1>
          <p class="subtitle">管理和编辑系统配置文件</p>
        </div>
        <div class="header-actions">
          <el-button
            type="success"
            :icon="Upload"
            @click="showImportDialog"
          >
            导入配置
          </el-button>
        </div>
      </div>
    </div>

    <!-- Main Content -->
    <el-row :gutter="20">
      <!-- Left Side: File List + Service Preview -->
      <el-col :span="8">
        <!-- Config Files List -->
        <el-card class="files-card">
          <template #header>
            <div class="card-header">
              <span>配置文件</span>
              <el-input
                v-model="searchQuery"
                placeholder="搜索配置..."
                size="small"
                :prefix-icon="Search"
                clearable
              />
            </div>
          </template>
          
          <div class="files-list">
            <div
              v-for="file in filteredConfigFiles"
              :key="file.name"
              class="file-item"
              :class="{ active: selectedFile?.name === file.name }"
              @click="selectFile(file)"
            >
              <div class="file-info">
                <div class="file-icon">
                  <el-icon><Document /></el-icon>
                </div>
                <div class="file-details">
                  <div class="file-name">{{ file.name }}</div>
                  <el-tag size="small" :type="file.type === 'json' ? 'primary' : 'info'">
                    {{ file.type.toUpperCase() }}
                  </el-tag>
                </div>
              </div>
            </div>
          </div>

          <template #footer>
            <div class="files-footer">
              <span>共 {{ configFiles.length }} 个配置文件</span>
            </div>
          </template>
        </el-card>

        <!-- Service Preview (Only for mcp.json) - Simple Summary -->
        <el-card v-if="isMcpJson" class="services-card">
          <template #header>
            <div class="card-header-row">
              <span>
                <el-icon><List /></el-icon>
                服务统计
              </span>
            </div>
          </template>
          
          <div class="service-stats">
            <el-statistic title="服务总数" :value="serviceCount">
              <template #suffix>个</template>
            </el-statistic>
            
            <el-divider />
            
            <div v-if="hasServices" class="service-names">
              <div class="stat-label">服务列表：</div>
              <el-tag 
                v-for="service in servicesList" 
                :key="service.name"
                size="small"
                class="service-tag"
              >
                {{ service.name }}
              </el-tag>
            </div>
            
            <el-empty v-else description="暂无服务配置" :image-size="60" />
          </div>
        </el-card>
      </el-col>

      <!-- Right Side: Editor -->
      <el-col :span="16">
        <el-card v-if="selectedFile" class="editor-card">
          <template #header>
            <div class="editor-header">
              <div class="file-info">
                <h3>{{ selectedFile.name }}</h3>
                <el-tag v-if="isModified" type="warning" size="small">已修改</el-tag>
                <el-tag v-if="hasErrors" type="danger" size="small">语法错误</el-tag>
                <el-tag v-if="!hasErrors && configText" type="success" size="small">语法正确</el-tag>
              </div>
              <div class="editor-actions">
                <!-- MCP JSON specific buttons -->
                <template v-if="isMcpJson">
                  <el-button
                    size="small"
                    :icon="View"
                    @click="showServicePreviewDialog"
                    :disabled="!hasServices"
                  >
                    服务预览
                  </el-button>
                  <el-button
                    size="small"
                    type="danger"
                    :icon="Delete"
                    @click="clearConfig"
                    plain
                  >
                    清空
                  </el-button>
                  <el-button
                    size="small"
                    type="warning"
                    :icon="RefreshLeft"
                    @click="resetConfig"
                    plain
                  >
                    重置
                  </el-button>
                </template>
                
                <!-- Common buttons -->
                <el-button
                  size="small"
                  :icon="Download"
                  @click="exportConfig"
                >
                  导出
                </el-button>
                <el-button
                  size="small"
                  type="success"
                  :icon="Check"
                  @click="saveConfig"
                  :loading="saving"
                  :disabled="hasErrors || !isModified"
                >
                  提交
                </el-button>
              </div>
            </div>
          </template>

          <!-- JSON Editor -->
          <div class="editor-container">
            <el-input
              v-model="configText"
              type="textarea"
              :rows="25"
              placeholder="请输入 JSON 配置..."
              @input="onConfigChange"
              class="json-editor"
            />
          </div>
        </el-card>

        <!-- Empty State -->
        <el-card v-else class="empty-card">
          <el-empty description="请从左侧选择一个配置文件" />
        </el-card>
      </el-col>
    </el-row>

    <!-- Import Dialog -->
    <el-dialog
      v-model="importDialogVisible"
      title="导入配置"
      width="500px"
    >
      <el-form label-width="100px">
        <el-form-item label="配置类型">
          <el-select v-model="importType" placeholder="选择配置类型">
            <el-option label="MCP JSON 配置" value="mcpjson" />
          </el-select>
        </el-form-item>
        
        <el-form-item label="选择文件">
          <el-upload
            ref="uploadRef"
            :auto-upload="false"
            :limit="1"
            accept=".json"
            :on-change="handleFileChange"
            :file-list="fileList"
          >
            <el-button :icon="Upload">选择 JSON 文件</el-button>
          </el-upload>
        </el-form-item>

        <el-alert
          title="注意：导入将覆盖当前配置"
          type="warning"
          :closable="false"
          show-icon
        />
      </el-form>

      <template #footer>
        <el-button @click="importDialogVisible = false">取消</el-button>
        <el-button
          type="primary"
          @click="confirmImport"
          :disabled="!selectedImportFile"
        >
          确认导入
        </el-button>
      </template>
    </el-dialog>

    <!-- Service Preview Dialog - Simplified -->
    <el-dialog
      v-model="previewDialogVisible"
      title="服务预览详情"
      width="700px"
    >
      <el-descriptions :column="2" border>
        <el-descriptions-item label="服务总数">
          {{ serviceCount }} 个
        </el-descriptions-item>
      </el-descriptions>
      
      <el-divider content-position="left">服务列表</el-divider>
      
      <div v-if="hasServices" class="preview-services">
        <el-card 
          v-for="(service, idx) in servicesList" 
          :key="service.name"
          class="preview-service-card"
          shadow="hover"
        >
          <div class="preview-header">
            <span class="preview-index">{{ idx + 1 }}.</span>
            <span class="preview-name">{{ service.name }}</span>
          </div>
          <div v-if="service.url" class="preview-detail">
            <span class="label">URL:</span>
            <span class="value">{{ service.url }}</span>
          </div>
          <div v-if="service.command" class="preview-detail">
            <span class="label">命令:</span>
            <span class="value">{{ service.command }}</span>
          </div>
          <div v-if="service.args && service.args.length" class="preview-detail">
            <span class="label">参数:</span>
            <span class="value">{{ service.args.join(' ') }}</span>
          </div>
        </el-card>
      </div>
      
      <el-empty v-else description="暂无服务" />

      <template #footer>
        <el-button @click="previewDialogVisible = false">关闭</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { api } from '@/api'
import {
  Document, Search, Upload, List, View, Delete, RefreshLeft,
  Download, Check
} from '@element-plus/icons-vue'

// State
const searchQuery = ref('')
const selectedFile = ref(null)
const configText = ref('')
const originalConfigText = ref('')
const isModified = ref(false)
const hasErrors = ref(false)
const saving = ref(false)
const resetting = ref(false)

// Import dialog
const importDialogVisible = ref(false)
const importType = ref('mcpjson')
const selectedImportFile = ref(null)
const fileList = ref([])
const uploadRef = ref(null)

// Service preview dialog
const previewDialogVisible = ref(false)

// Config files list
const configFiles = ref([
  { name: 'mcp.json', type: 'json', path: '/config/mcp.json' },
  // Add more config files here if needed
])

// Computed
const filteredConfigFiles = computed(() => {
  if (!searchQuery.value) return configFiles.value
  const query = searchQuery.value.toLowerCase()
  return configFiles.value.filter(file =>
    file.name.toLowerCase().includes(query)
  )
})

const isMcpJson = computed(() => {
  return selectedFile.value?.name === 'mcp.json'
})

// Parse mcp.json config
const parsedConfig = computed(() => {
  if (!isMcpJson.value) return null
  try {
    return JSON.parse(configText.value)
  } catch {
    return null
  }
})

const servicesList = computed(() => {
  if (!parsedConfig.value?.mcpServers) return []
  const servers = parsedConfig.value.mcpServers
  return Object.keys(servers).map(name => ({
    name,
    ...servers[name]
  }))
})

const serviceCount = computed(() => servicesList.value.length)
const hasServices = computed(() => serviceCount.value > 0)

// Methods
const selectFile = async (file) => {
  selectedFile.value = file
  
  if (file.name === 'mcp.json') {
    await loadMcpJson()
  } else {
    // Load other config files if needed
    configText.value = ''
    originalConfigText.value = ''
  }
}

const loadMcpJson = async () => {
  try {
    // getMcpJson() 已经通过 extractResponseData 提取了 data 字段
    const data = await api.store.getMcpJson()
    // data 就是 { mcpServers: {...} } 对象
    configText.value = typeof data === 'string' 
      ? data 
      : JSON.stringify(data, null, 2)
    originalConfigText.value = configText.value
    isModified.value = false
    validateJson()
  } catch (error) {
    ElMessage.error('加载配置失败: ' + error.message)
  }
}

const onConfigChange = () => {
  isModified.value = configText.value !== originalConfigText.value
  validateJson()
}

const validateJson = () => {
  try {
    if (configText.value.trim()) {
      JSON.parse(configText.value)
      hasErrors.value = false
    }
  } catch {
    hasErrors.value = true
  }
}

const saveConfig = async () => {
  if (hasErrors.value) {
    ElMessage.error('请先修复 JSON 语法错误')
    return
  }

  if (!isModified.value) {
    ElMessage.info('配置未修改')
    return
  }

  try {
    await ElMessageBox.confirm(
      '确定要提交配置吗？提交后会更新 mcp.json 文件并重新加载服务。',
      '确认提交',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )

    saving.value = true
    
    if (isMcpJson.value) {
      const config = JSON.parse(configText.value)
      await api.store.resetMcpJson(config)
      ElMessage.success('配置已提交，服务将自动重新加载')
      originalConfigText.value = configText.value
      isModified.value = false
    }
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error('提交失败: ' + error.message)
    }
  } finally {
    saving.value = false
  }
}

const clearConfig = async () => {
  try {
    await ElMessageBox.confirm('确定要清空所有配置吗？', '确认清空', {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      type: 'warning'
    })
    
    configText.value = JSON.stringify({ mcpServers: {} }, null, 2)
    isModified.value = true
    ElMessage.success('配置已清空（未保存）')
  } catch {
    // User cancelled
  }
}

const resetConfig = async () => {
  try {
    await ElMessageBox.confirm(
      '确定要重置配置吗？这将从服务器重新加载原始配置，本地未提交的修改将丢失。',
      '确认重置',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )
    
    resetting.value = true
    
    // TODO: 后续添加专门的重置接口
    // 目前使用重新加载来实现重置
    await loadMcpJson()
    ElMessage.success('配置已重置')
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error('重置失败: ' + error.message)
    }
  } finally {
    resetting.value = false
  }
}

const exportConfig = () => {
  if (!configText.value) {
    ElMessage.warning('没有可导出的内容')
    return
  }

  const blob = new Blob([configText.value], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = selectedFile.value?.name || 'config.json'
  link.click()
  URL.revokeObjectURL(url)
  ElMessage.success('配置已导出')
}

const showImportDialog = () => {
  importDialogVisible.value = true
  selectedImportFile.value = null
  fileList.value = []
}

const handleFileChange = (file) => {
  selectedImportFile.value = file
  fileList.value = [file]
}

const confirmImport = async () => {
  if (!selectedImportFile.value) return

  try {
    const reader = new FileReader()
    reader.onload = (e) => {
      try {
        const content = e.target.result
        JSON.parse(content) // Validate JSON
        configText.value = content
        isModified.value = true
        importDialogVisible.value = false
        ElMessage.success('配置已导入（未保存）')
        
        // Select mcp.json
        const mcpFile = configFiles.value.find(f => f.name === 'mcp.json')
        if (mcpFile) {
          selectedFile.value = mcpFile
        }
      } catch (error) {
        ElMessage.error('JSON 格式错误: ' + error.message)
      }
    }
    reader.readAsText(selectedImportFile.value.raw)
  } catch (error) {
    ElMessage.error('导入失败: ' + error.message)
  }
}

const showServicePreviewDialog = () => {
  previewDialogVisible.value = true
}

// Lifecycle
onMounted(() => {
  // Auto select mcp.json
  const mcpFile = configFiles.value.find(f => f.name === 'mcp.json')
  if (mcpFile) {
    selectFile(mcpFile)
  }
})
</script>

<style scoped>
.config-center {
  width: 92%;
  margin: 0 auto;
  max-width: none;
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

/* Files Card */
.files-card {
  margin-bottom: 20px;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
}

.files-list {
  max-height: 300px;
  overflow-y: auto;
}

.file-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
  margin-bottom: 8px;
}

.file-item:hover {
  background: var(--el-fill-color-light);
}

.file-item.active {
  background: var(--el-color-primary-light-9);
  border: 1px solid var(--el-color-primary);
}

.file-info {
  display: flex;
  align-items: center;
  gap: 12px;
  flex: 1;
}

.file-icon {
  font-size: 24px;
  color: var(--el-color-primary);
}

.file-details {
  flex: 1;
}

.file-name {
  font-weight: 600;
  margin-bottom: 4px;
}

.files-footer {
  text-align: center;
  color: var(--el-text-color-secondary);
  font-size: 13px;
}

/* Services Card */
.services-card {
  max-height: 450px;
}

.service-stats {
  padding: 10px 0;
}

.service-names {
  margin-top: 12px;
}

.stat-label {
  font-size: 13px;
  color: var(--el-text-color-secondary);
  margin-bottom: 8px;
}

.service-tag {
  margin: 4px 4px 4px 0;
}

.card-header-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

/* Editor Card */
.editor-card {
  min-height: 600px;
}

.editor-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.file-info {
  display: flex;
  align-items: center;
  gap: 12px;
}

.file-info h3 {
  margin: 0;
  font-size: 16px;
}

.editor-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.editor-container {
  margin-top: 16px;
}

.json-editor :deep(textarea) {
  font-family: 'Courier New', Consolas, monospace;
  font-size: 13px;
  line-height: 1.6;
}

.empty-card {
  min-height: 600px;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* Preview Dialog */
.preview-services {
  max-height: 400px;
  overflow-y: auto;
  margin-top: 12px;
}

.preview-service-card {
  margin-bottom: 12px;
}

.preview-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  font-weight: 600;
  font-size: 15px;
}

.preview-index {
  color: var(--el-color-primary);
}

.preview-name {
  color: var(--el-text-color-primary);
}

.preview-detail {
  font-size: 13px;
  margin-bottom: 6px;
  display: flex;
  gap: 8px;
}

.preview-detail .label {
  color: var(--el-text-color-secondary);
  min-width: 50px;
}

.preview-detail .value {
  color: var(--el-text-color-primary);
  word-break: break-all;
  flex: 1;
}
</style>

