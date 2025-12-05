<template>
  <div class="config-editor">
    <!-- Page Header -->
    <div class="page-header">
      <div class="header-content">
        <div class="header-title">
          <h1>高级配置编辑器</h1>
          <p class="subtitle">可视化编辑和管理系统配置文件</p>
        </div>
        <div class="header-actions">
          <el-button
            type="primary"
            :icon="Plus"
            @click="createNewConfig"
          >
            新建配置
          </el-button>
          <el-button
            type="success"
            :icon="Upload"
            @click="importConfig"
          >
            导入配置
          </el-button>
        </div>
      </div>
    </div>

    <!-- Config Files List -->
    <el-row :gutter="24">
      <el-col :span="8">
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
              :key="file.path"
              class="file-item"
              :class="{ active: selectedFile?.path === file.path }"
              @click="selectFile(file)"
            >
              <div class="file-info">
                <div class="file-icon">
                  <el-icon>
                    <Document v-if="file.type === 'json'" />
                    <Document v-else-if="file.type === 'yaml'" />
                    <Document v-else-if="file.type === 'toml'" />
                    <Document v-else />
                  </el-icon>
                </div>
                <div class="file-details">
                  <div class="file-name">{{ file.name }}</div>
                  <div class="file-path">{{ file.path }}</div>
                  <div class="file-meta">
                    <el-tag size="small" :type="file.type === 'json' ? 'primary' : 'info'">
                      {{ file.type.toUpperCase() }}
                    </el-tag>
                    <span class="file-size">{{ formatFileSize(file.size) }}</span>
                  </div>
                </div>
              </div>
              <div class="file-actions">
                <el-dropdown @command="(cmd) => handleFileAction(cmd, file)" trigger="click">
                  <el-button text size="small">
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

          <template #footer>
            <div class="files-footer">
              <span>共 {{ configFiles.length }} 个配置文件</span>
            </div>
          </template>
        </el-card>
      </el-col>

      <!-- Editor Panel -->
      <el-col :span="16">
        <el-card v-if="selectedFile" class="editor-card">
          <template #header>
            <div class="editor-header">
              <div class="file-info">
                <h3>{{ selectedFile.name }}</h3>
                <el-breadcrumb separator="/">
                  <el-breadcrumb-item>{{ selectedFile.type }}</el-breadcrumb-item>
                  <el-breadcrumb-item>{{ selectedFile.path }}</el-breadcrumb-item>
                </el-breadcrumb>
              </div>
              <div class="editor-actions">
                <el-button-group>
                  <el-button
                    size="small"
                    :type="viewMode === 'tree' ? 'primary' : 'default'"
                    @click="viewMode = 'tree'"
                  >
                    树形视图
                  </el-button>
                  <el-button
                    size="small"
                    :type="viewMode === 'code' ? 'primary' : 'default'"
                    @click="viewMode = 'code'"
                  >
                    代码视图
                  </el-button>
                </el-button-group>
                <el-button
                  size="small"
                  type="success"
                  :icon="Check"
                  @click="saveConfig"
                  :loading="saving"
                >
                  保存
                </el-button>
              </div>
            </div>
          </template>

          <!-- Tree View Editor -->
          <div v-if="viewMode === 'tree'" class="tree-editor">
            <div class="editor-toolbar">
              <el-button size="small" :icon="Plus" @click="addProperty">
                添加属性
              </el-button>
              <el-button size="small" :icon="FolderOpened" @click="addObject">
                添加对象
              </el-button>
              <el-button size="small" :icon="Collection" @click="addArray">
                添加数组
              </el-button>
              <el-divider direction="vertical" />
              <el-button size="small" :icon="RefreshLeft" @click="resetConfig">
                重置
              </el-button>
              <el-button size="small" :icon="MagicStick" @click="formatConfig">
                格式化
              </el-button>
            </div>

            <div class="tree-view">
              <ConfigTreeNode
                v-for="(node, key) in configData"
                :key="key"
                :node-key="key"
                :node-value="node"
                :depth="0"
                @update="handleNodeUpdate"
                @delete="handleNodeDelete"
              />
            </div>
          </div>

          <!-- Code Editor -->
          <div v-else class="code-editor">
            <el-input
              v-model="configText"
              type="textarea"
              :rows="20"
              placeholder="输入配置内容..."
              class="code-textarea"
              @input="handleCodeChange"
            />
          </div>

          <!-- Validation Panel -->
          <div class="validation-panel">
            <div class="validation-header">
              <span>配置验证</span>
              <el-tag :type="validation.isValid ? 'success' : 'danger'">
                {{ validation.isValid ? '验证通过' : '验证失败' }}
              </el-tag>
            </div>
            <div v-if="!validation.isValid" class="validation-errors">
              <div
                v-for="(error, index) in validation.errors"
                :key="index"
                class="error-item"
              >
                <el-icon><WarningFilled /></el-icon>
                <span>{{ error }}</span>
              </div>
            </div>
          </div>
        </el-card>

        <!-- Empty State -->
        <el-card v-else class="empty-card">
          <div class="empty-state">
            <el-icon class="empty-icon"><Document /></el-icon>
            <h3>选择配置文件</h3>
            <p>从左侧列表中选择一个配置文件进行编辑</p>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <!-- Schema Helper Dialog -->
    <el-dialog
      v-model="showSchemaDialog"
      title="配置架构参考"
      width="800px"
    >
      <el-tabs v-model="activeSchemaTab">
        <el-tab-pane label="MCP 配置" name="mcp">
          <div class="schema-content">
            <h4>MCP Server 配置架构</h4>
            <el-descriptions :column="1" border>
              <el-descriptions-item label="mcpServers">
                <code>object</code> - MCP服务器配置对象
              </el-descriptions-item>
              <el-descriptions-item label="serverName">
                <code>object</code> - 服务器配置
                <div class="schema-detail">
                  <div><strong>command</strong>: <code>string</code> - 启动命令</div>
                  <div><strong>args</strong>: <code>string[]</code> - 命令参数</div>
                  <div><strong>env</strong>: <code>object</code> - 环境变量</div>
                  <div><strong>cwd</strong>: <code>string</code> - 工作目录</div>
                </div>
              </el-descriptions-item>
            </el-descriptions>
          </div>
        </el-tab-pane>
        <el-tab-pane label="数据空间" name="dataspace">
          <div class="schema-content">
            <h4>数据空间配置架构</h4>
            <el-descriptions :column="1" border>
              <el-descriptions-item label="dataSpaces">
                <code>object</code> - 数据空间配置对象
              </el-descriptions-item>
              <el-descriptions-item label="spaceName">
                <code>object</code> - 空间配置
                <div class="schema-detail">
                  <div><strong>path</strong>: <code>string</code> - 存储路径</div>
                  <div><strong>maxSize</strong>: <code>string</code> - 最大大小</div>
                  <div><strong>retention</strong>: <code>string</code> - 保留策略</div>
                </div>
              </el-descriptions-item>
            </el-descriptions>
          </div>
        </el-tab-pane>
      </el-tabs>
      <template #footer>
        <el-button @click="showSchemaDialog = false">关闭</el-button>
      </template>
    </el-dialog>

    <!-- Import Dialog -->
    <el-dialog
      v-model="showImportDialog"
      title="导入配置"
      width="500px"
    >
      <el-form :model="importForm" label-width="80px">
        <el-form-item label="配置类型">
          <el-select v-model="importForm.type" placeholder="选择配置类型">
            <el-option label="MCP 配置" value="mcp" />
            <el-option label="数据空间" value="dataspace" />
            <el-option label="自定义" value="custom" />
          </el-select>
        </el-form-item>
        <el-form-item label="文件名">
          <el-input v-model="importForm.name" placeholder="输入配置文件名" />
        </el-form-item>
        <el-form-item label="配置内容">
          <el-input
            v-model="importForm.content"
            type="textarea"
            :rows="10"
            placeholder="粘贴配置内容..."
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showImportDialog = false">取消</el-button>
        <el-button type="primary" @click="confirmImport">导入</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, computed, watch, nextTick, h, defineComponent } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import yaml from 'js-yaml'
import toml from '@iarna/toml'
import {
  Plus, Upload, Search, Document, Edit, CopyDocument, Download, Delete,
  MoreFilled, Check, FolderOpened, Collection, RefreshLeft, MagicStick,
  WarningFilled, ArrowRight
} from '@element-plus/icons-vue'

// Tree Node Component
const ConfigTreeNode = defineComponent({
  name: 'ConfigTreeNode',
  props: {
    nodeKey: [String, Number],
    nodeValue: [Object, Array, String, Number, Boolean],
    depth: Number
  },
  emits: ['update', 'delete'],
  setup(props, { emit }) {
    const isExpanded = ref(true)
    const isEditing = ref(false)
    const editKey = ref(props.nodeKey)
    const editValue = ref('')

    const isObject = computed(() => typeof props.nodeValue === 'object' && !Array.isArray(props.nodeValue))
    const isArray = computed(() => Array.isArray(props.nodeValue))
    const isSimple = computed(() => !isObject.value && !isArray.value)

    const startEdit = () => {
      editKey.value = props.nodeKey
      editValue.value = typeof props.nodeValue === 'string' ? props.nodeValue : JSON.stringify(props.nodeValue)
      isEditing.value = true
    }

    const saveEdit = () => {
      emit('update', props.nodeKey, editKey.value, parseValue(editValue.value))
      isEditing.value = false
    }

    const parseValue = (val) => {
      try {
        return JSON.parse(val)
      } catch {
        return val
      }
    }

    const deleteNode = () => {
      emit('delete', props.nodeKey)
    }

    return () => {
      if (isEditing.value) {
        return h('div', { class: 'node-edit' }, [
          h('el-input', {
            modelValue: editKey.value,
            'onUpdate:modelValue': val => editKey.value = val,
            size: 'small',
            style: { width: '150px', marginRight: '8px' }
          }),
          h('el-input', {
            modelValue: editValue.value,
            'onUpdate:modelValue': val => editValue.value = val,
            size: 'small',
            style: { width: '200px', marginRight: '8px' }
          }),
          h('el-button', {
            size: 'small',
            type: 'primary',
            onClick: saveEdit
          }, { default: () => '保存' }),
          h('el-button', {
            size: 'small',
            onClick: () => isEditing.value = false
          }, { default: () => '取消' })
        ])
      }

      return h('div', { class: ['tree-node', `depth-${props.depth}`] }, [
        h('div', { class: 'node-header' }, [
          h('div', { class: 'node-toggle' }, [
            (isObject.value || isArray.value) && h('el-icon', {
              class: { 'is-expanded': isExpanded.value },
              onClick: () => isExpanded.value = !isExpanded.value
            }, { default: () => h(ArrowRight) })
          ]),
          h('div', { class: 'node-key' }, props.nodeKey),
          h('div', { class: 'node-type' }, [
            isObject.value && h('el-tag', { size: 'small' }, { default: () => 'Object' }),
            isArray.value && h('el-tag', { size: 'small', type: 'success' }, { default: () => 'Array' }),
            isSimple.value && h('el-tag', { size: 'small', type: 'info' }, { default: () => typeof props.nodeValue })
          ]),
          h('div', { class: 'node-value' }, [
            isSimple.value && h('code', {}, JSON.stringify(props.nodeValue))
          ]),
          h('div', { class: 'node-actions' }, [
            h('el-button', {
              size: 'small',
              text: true,
              icon: Edit,
              onClick: startEdit
            }),
            h('el-button', {
              size: 'small',
              text: true,
              icon: Delete,
              onClick: deleteNode
            })
          ])
        ]),
        isExpanded.value && (isObject.value || isArray.value) && h('div', { class: 'node-children' }, [
          Object.entries(props.nodeValue).map(([key, value]) => 
            h(ConfigTreeNode, {
              key,
              'node-key': key,
              'node-value': value,
              depth: props.depth + 1,
              onUpdate: emit('update'),
              onDelete: emit('delete')
            })
          )
        ])
      ])
    }
  }
})

// Main component setup
const searchQuery = ref('')
const selectedFile = ref(null)
const viewMode = ref('tree')
const saving = ref(false)
const configData = ref({})
const configText = ref('')
const validation = ref({ isValid: true, errors: [] })

// Dialog states
const showSchemaDialog = ref(false)
const showImportDialog = ref(false)
const activeSchemaTab = ref('mcp')

// Import form
const importForm = ref({
  type: '',
  name: '',
  content: ''
})

// Mock config files data
const configFiles = ref([
  {
    name: 'mcp.json',
    path: 'config/mcp.json',
    type: 'json',
    size: 2048,
    content: {
      mcpServers: {
        "filesystem": {
          command: "npx",
          args: ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/allowed/files"],
          env: {}
        },
        "git": {
          command: "npx",
          args: ["-y", "@modelcontextprotocol/server-git", "--repository", "/path/to/git/repo"],
          env: {}
        }
      }
    }
  },
  {
    name: 'dataspace.yaml',
    path: 'config/dataspace.yaml',
    type: 'yaml',
    size: 1024,
    content: {
      dataSpaces: {
        default: {
          path: './data/default',
          maxSize: '10GB',
          retention: '30d'
        },
        temp: {
          path: './data/temp',
          maxSize: '1GB',
          retention: '1d'
        }
      }
    }
  }
])

// Computed
const filteredConfigFiles = computed(() => {
  if (!searchQuery.value) return configFiles.value
  const query = searchQuery.value.toLowerCase()
  return configFiles.value.filter(file =>
    file.name.toLowerCase().includes(query) ||
    file.path.toLowerCase().includes(query)
  )
})

// Methods
const selectFile = (file) => {
  // Use nextTick to prevent infinite recursion
  nextTick(() => {
    selectedFile.value = file
    configData.value = JSON.parse(JSON.stringify(file.content))
    updateConfigText()
  })
}

const updateConfigText = () => {
  try {
    if (selectedFile.value?.type === 'json') {
      configText.value = JSON.stringify(configData.value, null, 2)
    } else if (selectedFile.value?.type === 'yaml') {
      configText.value = yaml.dump(configData.value)
    } else if (selectedFile.value?.type === 'toml') {
      configText.value = toml.stringify(configData.value)
    }
    validateConfig()
  } catch (error) {
    console.error('Failed to update config text:', error)
  }
}

const handleCodeChange = () => {
  try {
    configData.value = JSON.parse(configText.value)
    validateConfig()
  } catch {
    // Invalid JSON, continue editing
  }
}

const validateConfig = () => {
  const errors = []
  
  try {
    if (configText.value) {
      JSON.parse(configText.value)
    }
    validation.value = { isValid: true, errors: [] }
  } catch (error) {
    validation.value = {
      isValid: false,
      errors: [error.message]
    }
  }
}

const saveConfig = async () => {
  saving.value = true
  try {
    // Update the file content
    const fileIndex = configFiles.value.findIndex(f => f.path === selectedFile.value.path)
    if (fileIndex > -1) {
      configFiles.value[fileIndex].content = configData.value
      configFiles.value[fileIndex].size = new Blob([configText.value]).size
    }
    
    ElMessage.success('配置已保存')
  } catch (error) {
    console.error('Failed to save config:', error)
    ElMessage.error('保存失败')
  } finally {
    saving.value = false
  }
}

const createNewConfig = () => {
  const name = `config_${Date.now()}.json`
  const newFile = {
    name,
    path: `config/${name}`,
    type: 'json',
    size: 0,
    content: {}
  }
  configFiles.value.push(newFile)
  selectFile(newFile)
}

const importConfig = () => {
  importForm.value = { type: '', name: '', content: '' }
  showImportDialog.value = true
}

const confirmImport = () => {
  try {
    const content = JSON.parse(importForm.value.content)
    const newFile = {
      name: importForm.value.name,
      path: `config/${importForm.value.name}`,
      type: 'json',
      size: new Blob([importForm.value.content]).size,
      content
    }
    configFiles.value.push(newFile)
    showImportDialog.value = false
    ElMessage.success('配置导入成功')
  } catch (error) {
    ElMessage.error('配置格式错误')
  }
}

const handleFileAction = async (command, file) => {
  switch (command) {
    case 'edit':
      selectFile(file)
      break
    case 'duplicate':
      const copy = {
        ...file,
        name: `copy_${file.name}`,
        path: `config/copy_${file.name}`
      }
      configFiles.value.push(copy)
      ElMessage.success('已复制')
      break
    case 'export':
      const dataStr = JSON.stringify(file.content, null, 2)
      const dataBlob = new Blob([dataStr], { type: 'application/json' })
      const url = URL.createObjectURL(dataBlob)
      const link = document.createElement('a')
      link.href = url
      link.download = file.name
      link.click()
      URL.revokeObjectURL(url)
      ElMessage.success('已导出')
      break
    case 'delete':
      try {
        await ElMessageBox.confirm(`确定要删除 "${file.name}" 吗？`)
        const index = configFiles.value.findIndex(f => f.path === file.path)
        if (index > -1) {
          configFiles.value.splice(index, 1)
          // Use nextTick to prevent infinite recursion
          nextTick(() => {
            if (selectedFile.value?.path === file.path) {
              selectedFile.value = null
            }
          })
          ElMessage.success('已删除')
        }
      } catch {
        // Cancelled
      }
      break
  }
}

const addProperty = () => {
  const key = `new_property_${Object.keys(configData.value).length}`
  configData.value[key] = ''
  updateConfigText()
}

const addObject = () => {
  const key = `new_object_${Object.keys(configData.value).length}`
  configData.value[key] = {}
  updateConfigText()
}

const addArray = () => {
  const key = `new_array_${Object.keys(configData.value).length}`
  configData.value[key] = []
  updateConfigText()
}

const resetConfig = () => {
  configData.value = JSON.parse(JSON.stringify(selectedFile.value.content))
  updateConfigText()
}

const formatConfig = () => {
  updateConfigText()
  ElMessage.success('已格式化')
}

const handleNodeUpdate = (oldKey, newKey, newValue) => {
  if (oldKey !== newKey) {
    delete configData.value[oldKey]
  }
  configData.value[newKey] = newValue
  updateConfigText()
}

const handleNodeDelete = (key) => {
  delete configData.value[key]
  updateConfigText()
}

const formatFileSize = (bytes) => {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}

// Watch for file changes with debounce to prevent infinite recursion
watch(selectedFile, (newFile) => {
  if (newFile) {
    nextTick(() => {
      viewMode.value = 'tree'
    })
  }
})
</script>

<style lang="scss" scoped>
.config-editor {
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

/* Files Card */
.files-card {
  height: calc(100vh - 200px);
  display: flex;
  flex-direction: column;
  
  .files-list {
    flex: 1;
    overflow-y: auto;
  }
  
  .files-footer {
    text-align: center;
    color: var(--text-secondary);
    font-size: var(--font-size-sm);
  }
}

.file-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px;
  border-radius: var(--border-radius-md);
  cursor: pointer;
  transition: var(--transition-base);
  
  &:hover {
    background-color: var(--fill-color-lighter);
  }
  
  &.active {
    background-color: var(--primary-lighter);
    border: 1px solid var(--primary-color);
  }
}

.file-info {
  display: flex;
  align-items: center;
  gap: 12px;
  flex: 1;
  min-width: 0;
}

.file-icon {
  color: var(--primary-color);
}

.file-details {
  flex: 1;
  min-width: 0;
}

.file-name {
  font-weight: var(--font-weight-medium);
  margin-bottom: 4px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.file-path {
  font-size: var(--font-size-xs);
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.file-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 4px;
}

.file-size {
  font-size: var(--font-size-xs);
  color: var(--text-placeholder);
}

/* Editor Card */
.editor-card {
  height: calc(100vh - 200px);
  display: flex;
  flex-direction: column;
}

.editor-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  
  .file-info {
    h3 {
      margin: 0 0 4px 0;
      font-size: var(--font-size-lg);
    }
  }
  
  .editor-actions {
    display: flex;
    gap: 12px;
    align-items: center;
  }
}

.tree-editor {
  flex: 1;
  display: flex;
  flex-direction: column;
}

.editor-toolbar {
  padding: 12px 0;
  border-bottom: 1px solid var(--border-color);
  display: flex;
  gap: 8px;
}

.tree-view {
  flex: 1;
  overflow-y: auto;
  padding: 16px 0;
}

.tree-node {
  margin-bottom: 4px;
  
  .node-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px;
    border-radius: var(--border-radius-sm);
    
    &:hover {
      background-color: var(--fill-color-light);
    }
  }
  
  .node-toggle {
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    
    .el-icon {
      transition: transform var(--transition-fast);
      
      &.is-expanded {
        transform: rotate(90deg);
      }
    }
  }
  
  .node-key {
    font-weight: var(--font-weight-medium);
    min-width: 120px;
  }
  
  .node-value {
    flex: 1;
    code {
      background: var(--fill-color-lighter);
      padding: 2px 6px;
      border-radius: 4px;
      font-family: var(--font-family-mono);
    }
  }
  
  .node-actions {
    opacity: 0;
    transition: opacity var(--transition-fast);
  }
  
  &:hover .node-actions {
    opacity: 1;
  }
}

.node-children {
  margin-left: 32px;
  border-left: 1px dashed var(--border-color);
  padding-left: 16px;
}

.code-editor {
  flex: 1;
  display: flex;
  flex-direction: column;
  
  .code-textarea {
    flex: 1;
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    line-height: 1.5;
  }
}

.validation-panel {
  margin-top: 16px;
  padding: 12px;
  background: var(--fill-color-lighter);
  border-radius: var(--border-radius-md);
  
  .validation-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
  }
  
  .validation-errors {
    .error-item {
      display: flex;
      align-items: center;
      gap: 8px;
      color: var(--danger-color);
      font-size: var(--font-size-sm);
      margin-top: 4px;
    }
  }
}

/* Empty State */
.empty-card {
  height: calc(100vh - 200px);
  display: flex;
  align-items: center;
  justify-content: center;
}

.empty-state {
  text-align: center;
  
  .empty-icon {
    font-size: 64px;
    color: var(--text-placeholder);
    margin-bottom: 16px;
  }
  
  h3 {
    margin: 0 0 8px 0;
    color: var(--text-primary);
  }
  
  p {
    margin: 0;
    color: var(--text-secondary);
  }
}

/* Schema Content */
.schema-content {
  h4 {
    margin: 0 0 16px 0;
    color: var(--text-primary);
  }
  
  .schema-detail {
    margin-top: 8px;
    padding-left: 16px;
    
    div {
      margin: 4px 0;
      font-size: var(--font-size-sm);
      color: var(--text-regular);
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
  
  .el-row {
    flex-direction: column;
    
    .el-col {
      width: 100% !important;
      margin-bottom: 16px;
    }
  }
}
</style>