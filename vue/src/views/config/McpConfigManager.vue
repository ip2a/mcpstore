<template>
  <div class="mcp-config-manager">
    <div class="page-header">
      <h1>
        <el-icon><Document /></el-icon>
        MCP配置文件管理
      </h1>
      <p class="page-description">查看和编辑MCP JSON配置文件，管理服务配置</p>
    </div>

    <!-- 操作工具栏 -->
    <el-card class="toolbar-card">
      <div class="toolbar">
        <div class="toolbar-left">
          <el-button 
            type="primary" 
            :icon="Refresh" 
            @click="loadConfig"
            :loading="loading"
          >
            重新加载
          </el-button>
          
          <el-button 
            type="success" 
            :icon="Check" 
            @click="saveConfig"
            :loading="saving"
            :disabled="!isModified || hasErrors"
          >
            保存配置
          </el-button>
          

        </div>
        
        <div class="toolbar-right">
          <el-button 
            type="primary" 
            :icon="Upload" 
            @click="registerServices"
            :loading="registering"
            :disabled="hasErrors || !hasServices"
            size="large"
          >
            一键注册服务
          </el-button>
        </div>
      </div>
    </el-card>

    <!-- 主要内容区域 -->
    <el-row :gutter="20">
      <!-- JSON编辑器 -->
      <el-col :span="16">
        <el-card class="editor-card">
          <template #header>
            <div class="card-header">
              <span>
                <el-icon><Edit /></el-icon>
                配置文件编辑器
              </span>
              <div class="header-status">
                <el-tag v-if="isModified" type="warning" size="small">已修改</el-tag>
                <el-tag v-if="hasErrors" type="danger" size="small">语法错误</el-tag>
                <el-tag v-if="!hasErrors && configText" type="success" size="small">语法正确</el-tag>
              </div>
            </div>
          </template>
          
          <div class="editor-container">
            <JsonEditor
              v-model="configText"
              :height="500"
              @validate="onValidate"
              @change="onConfigChange"
            />
          </div>
        </el-card>
      </el-col>
      
      <!-- 侧边栏信息 -->
      <el-col :span="8">
        <!-- 文件信息 -->
        <el-card class="info-card">
          <template #header>
            <span>
              <el-icon><InfoFilled /></el-icon>
              文件信息
            </span>
          </template>
          
          <div class="file-info">
            <div class="info-item">
              <span class="label">文件状态:</span>
              <el-tag :type="isModified ? 'warning' : 'success'" size="small">
                {{ isModified ? '已修改' : '已保存' }}
              </el-tag>
            </div>
            
            <div class="info-item">
              <span class="label">JSON格式:</span>
              <el-tag :type="hasErrors ? 'danger' : 'success'" size="small">
                {{ hasErrors ? '格式错误' : '格式正确' }}
              </el-tag>
            </div>
            
            <div class="info-item">
              <span class="label">服务数量:</span>
              <span class="value">{{ serviceCount }} 个</span>
            </div>
            
            <div class="info-item">
              <span class="label">最后更新:</span>
              <span class="value">{{ lastUpdateTime || '未知' }}</span>
            </div>
          </div>
        </el-card>

        <!-- 服务预览 -->
        <el-card class="services-card">
          <template #header>
            <span>
              <el-icon><List /></el-icon>
              服务预览 ({{ serviceCount }})
            </span>
          </template>
          
          <div v-if="!hasServices" class="no-services">
            <el-empty description="暂无服务配置" :image-size="80" />
          </div>
          
          <div v-else class="services-list">
            <div 
              v-for="service in servicesList" 
              :key="service.name"
              class="service-item"
            >
              <div class="service-header">
                <span class="service-name">{{ service.name }}</span>
                <el-tag :type="getServiceTypeTag(service)" size="small">
                  {{ getServiceType(service) }}
                </el-tag>
              </div>
              
              <div class="service-details">
                <div v-if="service.command" class="detail-item">
                  <span class="detail-label">命令:</span>
                  <span class="detail-value">{{ service.command }}</span>
                </div>
                
                <div v-if="service.url" class="detail-item">
                  <span class="detail-label">URL:</span>
                  <span class="detail-value">{{ service.url }}</span>
                </div>
                
                <div v-if="service.args && service.args.length" class="detail-item">
                  <span class="detail-label">参数:</span>
                  <span class="detail-value">{{ service.args.join(' ') }}</span>
                </div>
              </div>
            </div>
          </div>
        </el-card>

        <!-- 快速操作 -->
        <el-card class="actions-card">
          <template #header>
            <span>
              <el-icon><Tools /></el-icon>
              快速操作
            </span>
          </template>
          
          <div class="quick-actions">
            <el-button 
              @click="addSampleService" 
              :icon="Plus" 
              type="primary" 
              plain 
              block
            >
              添加示例服务
            </el-button>
            
            <el-button 
              @click="clearConfig" 
              :icon="Delete" 
              type="danger" 
              plain 
              block
            >
              清空配置
            </el-button>
            
            <el-button 
              @click="resetToDefault" 
              :icon="RefreshLeft" 
              type="warning" 
              plain 
              block
            >
              重置为默认
            </el-button>
          </div>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  Document, Refresh, Check, Upload,
  Edit, InfoFilled, List, Tools, Plus, Delete, RefreshLeft
} from '@element-plus/icons-vue'
import { storeServiceAPI } from '@/api/services'
import JsonEditor from '@/components/config/JsonEditor.vue'

// 响应式数据
const loading = ref(false)
const saving = ref(false)
const registering = ref(false)
const configText = ref('')
const originalConfig = ref('')
const validationError = ref('')
const lastUpdateTime = ref('')

// 计算属性
const isModified = computed(() => configText.value !== originalConfig.value)
const hasErrors = computed(() => !!validationError.value)

const parsedConfig = computed(() => {
  if (hasErrors.value || !configText.value.trim()) return null
  try {
    return JSON.parse(configText.value)
  } catch {
    return null
  }
})

const servicesList = computed(() => {
  if (!parsedConfig.value?.mcpServers) return []
  return Object.entries(parsedConfig.value.mcpServers).map(([name, config]) => ({
    name,
    ...config
  }))
})

const serviceCount = computed(() => servicesList.value.length)
const hasServices = computed(() => serviceCount.value > 0)

// 方法
const loadConfig = async () => {
  loading.value = true
  try {
    const response = await storeServiceAPI.getConfig()
    console.log('🔍 完整API响应:', response)

    // 正确提取数据：response.data.data（API包装格式）
    let config = response.data?.data || response.data || { mcpServers: {} }
    console.log('🔍 提取的配置数据:', config)

    // 确保配置有正确的结构
    if (!config || typeof config !== 'object' || !config.mcpServers) {
      console.warn('⚠️ 配置格式不正确，使用默认结构')
      config = { mcpServers: {} }
    }

    configText.value = JSON.stringify(config, null, 2)
    originalConfig.value = configText.value
    lastUpdateTime.value = new Date().toLocaleString()

    console.log('🔍 最终配置文本:', configText.value)
    console.log('🔍 服务数量:', Object.keys(config.mcpServers || {}).length)

    ElMessage.success(`配置加载成功，包含 ${Object.keys(config.mcpServers || {}).length} 个服务`)
  } catch (error) {
    console.error('❌ 加载配置失败:', error)
    ElMessage.error(`加载配置失败: ${error.message}`)

    // 设置默认配置
    configText.value = JSON.stringify({ mcpServers: {} }, null, 2)
    originalConfig.value = configText.value
  } finally {
    loading.value = false
  }
}

const saveConfig = async () => {
  if (hasErrors.value) {
    ElMessage.error('请先修复JSON格式错误')
    return
  }

  saving.value = true
  try {
    const config = JSON.parse(configText.value)
    console.log('🔄 开始两步操作保存配置...')

    const response = await storeServiceAPI.updateConfigTwoStep(config)
    console.log('🔍 两步操作结果:', response.data)

    if (response.data.success) {
      originalConfig.value = configText.value
      lastUpdateTime.value = new Date().toLocaleString()
      ElMessage.success('配置保存成功：JSON文件已更新，服务已重新注册')
    } else {
      const result = response.data.data
      if (result.step1_json_update && !result.step2_service_registration) {
        ElMessage.warning(`部分成功：JSON文件已更新，但服务注册失败 - ${result.step2_error}`)
        originalConfig.value = configText.value
        lastUpdateTime.value = new Date().toLocaleString()
      } else if (!result.step1_json_update) {
        ElMessage.error(`JSON文件更新失败：${result.step1_error}`)
      } else {
        ElMessage.error(`保存失败: ${response.data.message}`)
      }
    }
  } catch (error) {
    console.error('❌ 保存配置失败:', error)
    ElMessage.error(`保存配置失败: ${error.message}`)
  } finally {
    saving.value = false
  }
}

const onValidate = (result) => {
  if (result.isValid) {
    validationError.value = ''
  } else {
    validationError.value = result.error || '配置验证失败'
  }
}

const onConfigChange = () => {
  // JsonEditor 组件会自动处理验证
}

const registerServices = async () => {
  if (hasErrors.value) {
    ElMessage.error('请先修复配置错误')
    return
  }
  
  if (!hasServices.value) {
    ElMessage.error('没有可注册的服务')
    return
  }
  
  try {
    await ElMessageBox.confirm(
      `确定要注册 ${serviceCount.value} 个服务吗？`,
      '确认注册',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )
    
    registering.value = true
    const config = JSON.parse(configText.value)
    
    // 先保存配置
    await storeServiceAPI.updateConfig(config)
    
    // 然后注册服务
    const serviceNames = Object.keys(config.mcpServers)
    await storeServiceAPI.addService(serviceNames)
    
    ElMessage.success(`成功注册 ${serviceNames.length} 个服务`)
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error(`注册服务失败: ${error.message}`)
    }
  } finally {
    registering.value = false
  }
}

const getServiceType = (service) => {
  if (service.url) return 'HTTP'
  if (service.command) return 'Command'
  return 'Unknown'
}

const getServiceTypeTag = (service) => {
  if (service.url) return 'primary'
  if (service.command) return 'success'
  return 'info'
}

const addSampleService = () => {
  const sampleService = {
    mcpServers: {
      "sample-service": {
        "command": "python",
        "args": ["sample_service.py"],
        "env": {
          "DEBUG": "true"
        }
      }
    }
  }
  
  try {
    const current = configText.value ? JSON.parse(configText.value) : { mcpServers: {} }
    current.mcpServers = { ...current.mcpServers, ...sampleService.mcpServers }
    configText.value = JSON.stringify(current, null, 2)
    ElMessage.success('示例服务已添加')
  } catch (error) {
    ElMessage.error('添加示例服务失败')
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
    ElMessage.success('配置已清空')
  } catch (error) {
    // 用户取消
  }
}

const resetToDefault = async () => {
  try {
    await ElMessageBox.confirm('确定要重置为默认配置吗？', '确认重置', {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      type: 'warning'
    })
    
    configText.value = originalConfig.value
    ElMessage.success('已重置为原始配置')
  } catch (error) {
    // 用户取消
  }
}

// 生命周期
onMounted(() => {
  loadConfig()
})

// JsonEditor组件会自动处理验证，不需要额外的监听器
</script>

<style scoped>
.mcp-config-manager {
  padding: 20px;
  max-width: 1400px;
  margin: 0 auto;
}

.page-header {
  margin-bottom: 20px;
}

.page-header h1 {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0 0 8px 0;
  color: #303133;
}

.page-description {
  color: #606266;
  margin: 0;
}

.toolbar-card {
  margin-bottom: 20px;
}

.toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.toolbar-left {
  display: flex;
  gap: 10px;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.header-status {
  display: flex;
  gap: 8px;
}

.editor-container {
  position: relative;
}

.json-editor {
  font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
  font-size: 14px;
}

.json-editor.has-error :deep(.el-textarea__inner) {
  border-color: #f56c6c;
}

.error-message {
  margin-top: 10px;
}

.info-card,
.services-card,
.actions-card {
  margin-bottom: 20px;
}

.file-info {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.info-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.label {
  font-weight: 500;
  color: #606266;
}

.value {
  color: #303133;
}

.no-services {
  text-align: center;
  padding: 20px;
}

.services-list {
  max-height: 300px;
  overflow-y: auto;
}

.service-item {
  padding: 12px;
  border: 1px solid #ebeef5;
  border-radius: 4px;
  margin-bottom: 8px;
}

.service-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}

.service-name {
  font-weight: 500;
  color: #303133;
}

.service-details {
  font-size: 12px;
  color: #606266;
}

.detail-item {
  margin-bottom: 4px;
}

.detail-label {
  font-weight: 500;
  margin-right: 4px;
}

.detail-value {
  word-break: break-all;
}

.quick-actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
</style>
