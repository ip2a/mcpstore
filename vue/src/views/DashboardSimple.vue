<template>
  <div class="dashboard-simple">
    <h1>🧪 简化仪表板测试</h1>
    
    <!-- 调试信息 -->
    <el-card class="debug-card">
      <template #header>
        <span>🔍 调试信息</span>
      </template>
      
      <div class="debug-info">
        <p><strong>环境模式:</strong> {{ currentMode }}</p>
        <p><strong>API地址:</strong> {{ apiBaseUrl }}</p>
        <p><strong>当前路径:</strong> {{ currentPath }}</p>
        <p><strong>错误状态:</strong> {{ hasError }}</p>
        <p><strong>加载状态:</strong> {{ isLoading }}</p>
      </div>
    </el-card>

    <!-- 错误状态显示 -->
    <el-card v-if="hasError" class="error-card">
      <template #header>
        <span>❌ 错误信息</span>
      </template>
      
      <div class="error-info">
        <p><strong>错误类型:</strong> {{ errorType }}</p>
        <p><strong>错误标题:</strong> {{ errorTitle }}</p>
        <p><strong>错误描述:</strong> {{ errorDescription }}</p>
        <el-button @click="handleRetry" type="primary">重试</el-button>
      </div>
    </el-card>

    <!-- 正常内容 -->
    <div v-else>
      <!-- 基本统计 -->
      <el-row :gutter="20">
        <el-col :span="6">
          <el-card class="stat-card">
            <div class="stat-content">
              <div class="stat-number">{{ serviceStats.total }}</div>
              <div class="stat-label">总服务数</div>
            </div>
          </el-card>
        </el-col>
        
        <el-col :span="6">
          <el-card class="stat-card">
            <div class="stat-content">
              <div class="stat-number">{{ serviceStats.healthy }}</div>
              <div class="stat-label">健康服务</div>
            </div>
          </el-card>
        </el-col>
        
        <el-col :span="6">
          <el-card class="stat-card">
            <div class="stat-content">
              <div class="stat-number">{{ toolStats.available }}</div>
              <div class="stat-label">可用工具</div>
            </div>
          </el-card>
        </el-col>
        
        <el-col :span="6">
          <el-card class="stat-card">
            <div class="stat-content">
              <div class="stat-number">{{ agentStats.total }}</div>
              <div class="stat-label">Agent数量</div>
            </div>
          </el-card>
        </el-col>
      </el-row>

      <!-- 操作按钮 -->
      <el-card class="action-card">
        <template #header>
          <span>🛠️ 操作</span>
        </template>
        
        <div class="actions">
          <el-button @click="loadData" :loading="isLoading">刷新数据</el-button>
          <el-button @click="testApi">测试API</el-button>
          <el-button @click="clearErrors">清除错误</el-button>
        </div>
      </el-card>

      <!-- 数据显示 -->
      <el-card class="data-card">
        <template #header>
          <span>📊 数据详情</span>
        </template>
        
        <div class="data-content">
          <h4>服务列表 ({{ services.length }})</h4>
          <ul>
            <li v-for="service in services.slice(0, 5)" :key="service.name">
              {{ service.name }} - {{ service.status }}
            </li>
          </ul>
          
          <h4>工具列表 ({{ tools.length }})</h4>
          <ul>
            <li v-for="tool in tools.slice(0, 5)" :key="tool.name">
              {{ tool.name }}
            </li>
          </ul>
        </div>
      </el-card>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import { useAppStore } from '@/stores/app'
import { useSystemStore } from '@/stores/system'
import { useServicesStore } from '@/stores/services'
import { useToolsStore } from '@/stores/tools'

// Store实例
const appStore = useAppStore()
const systemStore = useSystemStore()
const servicesStore = useServicesStore()
const toolsStore = useToolsStore()

// 响应式数据
const hasLocalError = ref(false)
const errorType = ref('')
const errorTitle = ref('')
const errorDescription = ref('')

// 计算属性
const currentMode = computed(() => import.meta.env.MODE)
const apiBaseUrl = computed(() => import.meta.env.VITE_API_BASE_URL)
const currentPath = computed(() => window.location.pathname)

const hasError = computed(() => 
  hasLocalError.value || appStore.hasErrors || systemStore.hasErrors || 
  servicesStore.hasErrors || toolsStore.hasErrors
)

const isLoading = computed(() => 
  appStore.isLoading || systemStore.isLoading || 
  servicesStore.isLoading || toolsStore.isLoading
)

const services = computed(() => servicesStore.services)
const tools = computed(() => toolsStore.tools)

const serviceStats = computed(() => ({
  total: services.value.length,
  healthy: services.value.filter(s => s.status === 'healthy').length
}))

const toolStats = computed(() => ({
  available: tools.value.length
}))

const agentStats = computed(() => ({
  total: 0 // 简化版本
}))

// 方法
const loadData = async () => {
  try {
    hasLocalError.value = false
    
    console.log('🔍 [简化版] 开始加载数据...')
    
    // 清除所有错误
    appStore.clearErrors()
    systemStore.clearErrors()
    servicesStore.clearErrors()
    toolsStore.clearErrors()
    
    // 加载数据
    await Promise.all([
      servicesStore.fetchServices(true),
      toolsStore.fetchTools(true)
    ])
    
    console.log('✅ [简化版] 数据加载成功:', {
      services: services.value.length,
      tools: tools.value.length
    })
    
    ElMessage.success('数据加载成功')
    
  } catch (error) {
    console.error('❌ [简化版] 数据加载失败:', error)
    handleError(error)
  }
}

const testApi = async () => {
  try {
    const response = await fetch(`${apiBaseUrl.value}/health`)
    if (response.ok) {
      const data = await response.json()
      ElMessage.success('API连接正常')
      console.log('API测试成功:', data)
    } else {
      throw new Error(`HTTP ${response.status}`)
    }
  } catch (error) {
    ElMessage.error(`API连接失败: ${error.message}`)
    console.error('API测试失败:', error)
  }
}

const clearErrors = () => {
  hasLocalError.value = false
  appStore.clearErrors()
  systemStore.clearErrors()
  servicesStore.clearErrors()
  toolsStore.clearErrors()
  ElMessage.success('错误状态已清除')
}

const handleError = (error) => {
  hasLocalError.value = true
  errorType.value = 'unknown'
  errorTitle.value = '加载失败'
  errorDescription.value = `数据加载失败: ${error.message}`
  
  console.error('处理错误:', error)
}

const handleRetry = () => {
  loadData()
}

// 生命周期
onMounted(() => {
  console.log('🧪 简化仪表板组件已挂载')
  loadData()
})
</script>

<style scoped>
.dashboard-simple {
  padding: 20px;
  max-width: 1200px;
  margin: 0 auto;
}

.debug-card,
.error-card,
.action-card,
.data-card {
  margin-bottom: 20px;
}

.debug-info p,
.error-info p {
  margin: 8px 0;
  font-family: monospace;
}

.stat-card {
  text-align: center;
}

.stat-content {
  padding: 20px;
}

.stat-number {
  font-size: 2em;
  font-weight: bold;
  color: #409eff;
}

.stat-label {
  margin-top: 8px;
  color: #666;
}

.actions {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
}

.data-content h4 {
  margin: 15px 0 10px 0;
  color: #333;
}

.data-content ul {
  margin: 0 0 20px 20px;
}

.data-content li {
  margin: 5px 0;
  font-family: monospace;
}
</style>
