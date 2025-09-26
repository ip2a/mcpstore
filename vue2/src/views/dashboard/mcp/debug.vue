<template>
  <div class="debug-page">
    <h2>MCP API 调试页面</h2>
    
    <el-card class="mb-4">
      <template #header>
        <div class="card-header">
          <span>API 连接测试</span>
          <el-button type="primary" @click="testAllApis">测试所有 API</el-button>
        </div>
      </template>
      
      <div class="api-test-results">
        <div v-for="(result, key) in apiResults" :key="key" class="api-result">
          <h4>{{ key }}</h4>
          <el-tag :type="result.success ? 'success' : 'danger'">
            {{ result.success ? '成功' : '失败' }}
          </el-tag>
          <pre class="result-data">{{ JSON.stringify(result.data, null, 2) }}</pre>
        </div>
      </div>
    </el-card>
    
    <el-card>
      <template #header>
        <span>仪表盘数据状态</span>
      </template>
      
      <div class="dashboard-data">
        <div class="data-section">
          <h4>服务数据</h4>
          <pre>{{ JSON.stringify(services, null, 2) }}</pre>
        </div>
        
        <div class="data-section">
          <h4>工具数据</h4>
          <pre>{{ JSON.stringify(toolsData, null, 2) }}</pre>
        </div>
        
        <div class="data-section">
          <h4>统计卡片数据</h4>
          <pre>{{ JSON.stringify(statCards, null, 2) }}</pre>
        </div>
      </div>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import { dashboardApi } from '@/mcp/api/dashboard'
import { useDashboardData } from '@/composables/useDashboardData'

// 使用仪表盘数据
const {
  services,
  toolsData,
  toolRecords,
  systemResources,
  agentsSummary,
  statCards
} = useDashboardData()

// API 测试结果
const apiResults = ref<Record<string, any>>({})

// 测试单个 API
const testApi = async (name: string, apiCall: () => Promise<any>) => {
  try {
    const result = await apiCall()
    apiResults.value[name] = {
      success: true,
      data: result
    }
    ElMessage.success(`${name} API 测试成功`)
  } catch (error) {
    apiResults.value[name] = {
      success: false,
      data: error
    }
    ElMessage.error(`${name} API 测试失败`)
    console.error(`${name} API Error:`, error)
  }
}

// 测试所有 API
const testAllApis = async () => {
  ElMessage.info('开始测试所有 API...')
  
  await testApi('服务列表', dashboardApi.getServices)
  await testApi('工具列表', dashboardApi.getTools)
  await testApi('工具记录', () => dashboardApi.getToolRecords(10))
  await testApi('系统资源', dashboardApi.getSystemResources)
  await testApi('Agent统计', dashboardApi.getAgentsSummary)
  await testApi('健康状态', dashboardApi.getHealthSummary)
  
  ElMessage.success('API 测试完成')
}

onMounted(() => {
  console.log('Debug page mounted')
})
</script>

<style lang="scss" scoped>
.debug-page {
  padding: 20px;
  
  .mb-4 {
    margin-bottom: 20px;
  }
  
  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  
  .api-result {
    margin-bottom: 20px;
    padding: 15px;
    border: 1px solid #eee;
    border-radius: 4px;
    
    h4 {
      margin: 0 0 10px 0;
      color: #333;
    }
    
    .result-data {
      margin-top: 10px;
      padding: 10px;
      background: #f5f5f5;
      border-radius: 4px;
      font-size: 12px;
      max-height: 300px;
      overflow-y: auto;
    }
  }
  
  .data-section {
    margin-bottom: 20px;
    
    h4 {
      margin: 0 0 10px 0;
      color: #333;
    }
    
    pre {
      padding: 10px;
      background: #f5f5f5;
      border-radius: 4px;
      font-size: 12px;
      max-height: 200px;
      overflow-y: auto;
    }
  }
}
</style>
