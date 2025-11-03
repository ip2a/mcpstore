<template>
  <div class="api-debug">
    <h1>🔍 API调试页面</h1>
    
    <el-card class="debug-card">
      <template #header>
        <span>API测试</span>
      </template>
      
      <div class="test-buttons">
        <el-button @click="testServicesAPI" :loading="testing">测试Services API</el-button>
        <el-button @click="testToolsAPI" :loading="testing">测试Tools API</el-button>
        <el-button @click="testHealthAPI" :loading="testing">测试Health API</el-button>
        <el-button @click="clearResults">清除结果</el-button>
      </div>
    </el-card>

    <el-card v-if="results.length > 0" class="results-card">
      <template #header>
        <span>测试结果</span>
      </template>
      
      <div v-for="(result, index) in results" :key="index" class="result-item">
        <h4>{{ result.title }}</h4>
        <div class="result-status" :class="result.success ? 'success' : 'error'">
          {{ result.success ? '✅ 成功' : '❌ 失败' }}
        </div>
        <div class="result-details">
          <h5>原始响应:</h5>
          <pre>{{ JSON.stringify(result.response, null, 2) }}</pre>
          
          <h5>数据类型分析:</h5>
          <ul>
            <li>响应类型: {{ result.analysis.responseType }}</li>
            <li>是否有data字段: {{ result.analysis.hasData }}</li>
            <li>data类型: {{ result.analysis.dataType }}</li>
            <li>data是否为数组: {{ result.analysis.isDataArray }}</li>
            <li>响应是否为数组: {{ result.analysis.isResponseArray }}</li>
            <li>数据长度: {{ result.analysis.dataLength }}</li>
          </ul>
          
          <h5>建议处理方式:</h5>
          <p>{{ result.analysis.suggestion }}</p>
        </div>
      </div>
    </el-card>
  </div>
</template>

<script setup>
import { ref } from 'vue'
import { api } from '@/api'

const testing = ref(false)
const results = ref([])

const analyzeResponse = (response) => {
  const analysis = {
    responseType: typeof response,
    hasData: response && typeof response === 'object' && 'data' in response,
    dataType: response?.data ? typeof response.data : 'undefined',
    isDataArray: Array.isArray(response?.data),
    isResponseArray: Array.isArray(response),
    dataLength: response?.data?.length || (Array.isArray(response) ? response.length : 0)
  }
  
  // 生成建议
  if (analysis.isDataArray) {
    analysis.suggestion = '使用 response.data (已经是数组)'
  } else if (analysis.isResponseArray) {
    analysis.suggestion = '使用 response (响应本身是数组)'
  } else if (response?.data?.services && Array.isArray(response.data.services)) {
    analysis.suggestion = '使用 response.data.services'
  } else if (response?.data?.tools && Array.isArray(response.data.tools)) {
    analysis.suggestion = '使用 response.data.tools'
  } else {
    analysis.suggestion = '数据格式不符合预期，使用空数组作为fallback'
  }
  
  return analysis
}

const testServicesAPI = async () => {
  testing.value = true
  try {
    console.log('🔍 测试Services API...')
    const response = await api.store.listServices()
    
    const analysis = analyzeResponse(response)
    
    results.value.push({
      title: 'Services API (/for_store/list_services)',
      success: true,
      response,
      analysis
    })
    
    console.log('✅ Services API测试成功:', response)
  } catch (error) {
    results.value.push({
      title: 'Services API (/for_store/list_services)',
      success: false,
      response: { error: error.message, stack: error.stack },
      analysis: { suggestion: '请求失败，检查网络连接和后端服务' }
    })
    console.error('❌ Services API测试失败:', error)
  } finally {
    testing.value = false
  }
}

const testToolsAPI = async () => {
  testing.value = true
  try {
    console.log('🔍 测试Tools API...')
    const response = await api.store.getTools()
    
    const analysis = analyzeResponse(response)
    
    results.value.push({
      title: 'Tools API (/for_store/list_tools)',
      success: true,
      response,
      analysis
    })
    
    console.log('✅ Tools API测试成功:', response)
  } catch (error) {
    results.value.push({
      title: 'Tools API (/for_store/list_tools)',
      success: false,
      response: { error: error.message, stack: error.stack },
      analysis: { suggestion: '请求失败，检查网络连接和后端服务' }
    })
    console.error('❌ Tools API测试失败:', error)
  } finally {
    testing.value = false
  }
}

const testHealthAPI = async () => {
  testing.value = true
  try {
    console.log('🔍 测试Health API...')
    const response = await fetch(`${import.meta.env.VITE_API_BASE_URL}/health`)
    const data = await response.json()
    
    results.value.push({
      title: 'Health API (/health)',
      success: response.ok,
      response: data,
      analysis: {
        responseType: typeof data,
        suggestion: response.ok ? '健康检查正常' : '健康检查失败'
      }
    })
    
    console.log('✅ Health API测试成功:', data)
  } catch (error) {
    results.value.push({
      title: 'Health API (/health)',
      success: false,
      response: { error: error.message },
      analysis: { suggestion: '健康检查失败，后端服务可能未启动' }
    })
    console.error('❌ Health API测试失败:', error)
  } finally {
    testing.value = false
  }
}

const clearResults = () => {
  results.value = []
}
</script>

<style scoped>
.api-debug {
  padding: 20px;
  max-width: 1200px;
  margin: 0 auto;
}

.debug-card,
.results-card {
  margin-bottom: 20px;
}

.test-buttons {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
}

.result-item {
  border: 1px solid #ddd;
  border-radius: 4px;
  padding: 15px;
  margin-bottom: 15px;
}

.result-item h4 {
  margin: 0 0 10px 0;
  color: #333;
}

.result-status {
  font-weight: bold;
  margin-bottom: 15px;
}

.result-status.success {
  color: #67c23a;
}

.result-status.error {
  color: #f56c6c;
}

.result-details h5 {
  margin: 15px 0 5px 0;
  color: #666;
}

.result-details pre {
  background: #f5f5f5;
  padding: 10px;
  border-radius: 4px;
  overflow-x: auto;
  font-size: 12px;
  max-height: 300px;
}

.result-details ul {
  margin: 5px 0 5px 20px;
}

.result-details li {
  margin: 3px 0;
  font-family: monospace;
}

.result-details p {
  background: #e6f7ff;
  padding: 8px;
  border-radius: 4px;
  margin: 5px 0;
  font-weight: bold;
}
</style>
