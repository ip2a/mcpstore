<template>
  <div class="test-page">
    <h1>🧪 环境测试页面</h1>
    
    <el-card class="test-card">
      <template #header>
        <span>环境信息</span>
      </template>
      
      <div class="test-info">
        <p><strong>当前模式:</strong> {{ currentMode }}</p>
        <p><strong>API地址:</strong> {{ apiBaseUrl }}</p>
        <p><strong>Base URL:</strong> {{ baseUrl }}</p>
        <p><strong>当前路径:</strong> {{ currentPath }}</p>
        <p><strong>完整URL:</strong> {{ fullUrl }}</p>
      </div>
    </el-card>

    <el-card class="test-card">
      <template #header>
        <span>API连接测试</span>
      </template>
      
      <div class="test-actions">
        <el-button @click="testApiConnection" :loading="testing">
          测试API连接
        </el-button>
        <el-button @click="testStoreServices" :loading="testing">
          测试服务列表
        </el-button>
      </div>
      
      <div v-if="testResult" class="test-result">
        <h4>测试结果:</h4>
        <pre>{{ testResult }}</pre>
      </div>
    </el-card>

    <el-card class="test-card">
      <template #header>
        <span>路由测试</span>
      </template>
      
      <div class="test-actions">
        <el-button @click="goToDashboard">跳转到仪表板</el-button>
        <el-button @click="goToServices">跳转到服务列表</el-button>
        <el-button @click="goToTools">跳转到工具列表</el-button>
      </div>
    </el-card>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { storeServiceAPI } from '@/api/services'

const router = useRouter()

// 响应式数据
const testing = ref(false)
const testResult = ref('')

// 计算属性
const currentMode = computed(() => import.meta.env.MODE)
const apiBaseUrl = computed(() => import.meta.env.VITE_API_BASE_URL)
const baseUrl = computed(() => import.meta.env.BASE_URL)
const currentPath = computed(() => window.location.pathname)
const fullUrl = computed(() => window.location.href)

// 方法
const testApiConnection = async () => {
  testing.value = true
  testResult.value = ''
  
  try {
    // 测试基本连接
    const response = await fetch(`${apiBaseUrl.value}/health`, {
      method: 'GET',
      headers: {
        'Content-Type': 'application/json'
      }
    })
    
    if (response.ok) {
      const data = await response.json()
      testResult.value = `✅ API连接成功!\n${JSON.stringify(data, null, 2)}`
    } else {
      testResult.value = `❌ API连接失败: ${response.status} ${response.statusText}`
    }
  } catch (error) {
    testResult.value = `❌ API连接错误: ${error.message}`
  } finally {
    testing.value = false
  }
}

const testStoreServices = async () => {
  testing.value = true
  testResult.value = ''
  
  try {
    const response = await storeServiceAPI.getServices()
    testResult.value = `✅ 服务列表获取成功!\n${JSON.stringify(response, null, 2)}`
  } catch (error) {
    testResult.value = `❌ 服务列表获取失败: ${error.message}`
  } finally {
    testing.value = false
  }
}

const goToDashboard = () => {
  router.push('/dashboard')
}

const goToServices = () => {
  router.push('/services')
}

const goToTools = () => {
  router.push('/tools')
}

// 生命周期
onMounted(() => {
  console.log('🧪 测试页面加载完成')
  console.log('环境信息:', {
    mode: currentMode.value,
    apiBaseUrl: apiBaseUrl.value,
    baseUrl: baseUrl.value,
    currentPath: currentPath.value,
    fullUrl: fullUrl.value
  })
})
</script>

<style scoped>
.test-page {
  padding: 20px;
  max-width: 800px;
  margin: 0 auto;
}

.test-card {
  margin-bottom: 20px;
}

.test-info p {
  margin: 8px 0;
  font-family: monospace;
}

.test-actions {
  margin-bottom: 20px;
}

.test-actions .el-button {
  margin-right: 10px;
  margin-bottom: 10px;
}

.test-result {
  margin-top: 20px;
  padding: 15px;
  background-color: #f5f5f5;
  border-radius: 4px;
}

.test-result pre {
  white-space: pre-wrap;
  word-wrap: break-word;
  font-family: monospace;
  font-size: 12px;
}
</style>
