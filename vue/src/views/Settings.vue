<template>
  <div class="settings">
    <!-- 页面头部 -->
    <div class="page-header">
      <div class="header-left">
        <h2 class="page-title">系统设置</h2>
        <p class="page-description">配置系统参数和用户偏好</p>
      </div>
      <div class="header-right">
        <el-button 
          type="primary" 
          @click="saveSettings"
          :loading="saving"
        >
          保存设置
        </el-button>
        <el-button 
          @click="resetSettings"
        >
          重置设置
        </el-button>
      </div>
    </div>
    
    <!-- 设置选项卡 -->
    <el-tabs v-model="activeTab" class="settings-tabs">
      <!-- 基础设置 -->
      <el-tab-pane label="基础设置" name="basic">
        <el-card class="settings-card">
          <el-form :model="basicSettings" label-width="150px">
            <el-form-item label="系统名称">
              <el-input 
                v-model="basicSettings.systemName" 
                placeholder="MCPStore 管理面板"
              />
            </el-form-item>
            
            <el-form-item label="API地址">
              <el-input 
                v-model="basicSettings.apiUrl" 
                placeholder="http://localhost:18200"
              />
            </el-form-item>
            
            <el-form-item label="请求超时">
              <el-input-number
                v-model="basicSettings.timeout"
                :min="5000"
                :max="60000"
                :step="1000"
                style="width: 200px"
              />
              <span class="unit">毫秒</span>
            </el-form-item>
            
            <el-form-item label="自动刷新间隔">
              <el-input-number
                v-model="basicSettings.refreshInterval"
                :min="10"
                :max="300"
                :step="10"
                style="width: 200px"
              />
              <span class="unit">秒</span>
            </el-form-item>
            
            <el-form-item label="语言设置">
              <el-select v-model="basicSettings.language" style="width: 200px">
                <el-option label="简体中文" value="zh-CN" />
                <el-option label="English" value="en-US" />
              </el-select>
            </el-form-item>
          </el-form>
        </el-card>
      </el-tab-pane>
      
      <!-- 界面设置 -->
      <el-tab-pane label="界面设置" name="ui">
        <el-card class="settings-card">
          <el-form :model="uiSettings" label-width="150px">
            <el-form-item label="主题模式">
              <el-radio-group v-model="uiSettings.theme">
                <el-radio label="light">亮色主题</el-radio>
                <el-radio label="dark">暗色主题</el-radio>
                <el-radio label="auto">跟随系统</el-radio>
              </el-radio-group>
            </el-form-item>
            
            <el-form-item label="主色调">
              <el-color-picker 
                v-model="uiSettings.primaryColor"
                show-alpha
                :predefine="predefineColors"
              />
            </el-form-item>
            
            <el-form-item label="侧边栏">
              <el-checkbox v-model="uiSettings.sidebarCollapsed">默认收起侧边栏</el-checkbox>
            </el-form-item>
            
            <el-form-item label="面包屑导航">
              <el-checkbox v-model="uiSettings.showBreadcrumb">显示面包屑导航</el-checkbox>
            </el-form-item>
            
            <el-form-item label="页面动画">
              <el-checkbox v-model="uiSettings.enableAnimation">启用页面切换动画</el-checkbox>
            </el-form-item>
            
            <el-form-item label="表格密度">
              <el-radio-group v-model="uiSettings.tableDensity">
                <el-radio label="large">宽松</el-radio>
                <el-radio label="default">默认</el-radio>
                <el-radio label="small">紧凑</el-radio>
              </el-radio-group>
            </el-form-item>
          </el-form>
        </el-card>
      </el-tab-pane>
      
      <!-- 通知设置 -->
      <el-tab-pane label="通知设置" name="notification">
        <el-card class="settings-card">
          <el-form :model="notificationSettings" label-width="150px">
            <el-form-item label="桌面通知">
              <el-checkbox v-model="notificationSettings.desktop">启用桌面通知</el-checkbox>
            </el-form-item>
            
            <el-form-item label="声音提示">
              <el-checkbox v-model="notificationSettings.sound">启用声音提示</el-checkbox>
            </el-form-item>
            
            <el-form-item label="服务异常通知">
              <el-checkbox v-model="notificationSettings.serviceError">服务异常时通知</el-checkbox>
            </el-form-item>
            
            <el-form-item label="工具执行通知">
              <el-checkbox v-model="notificationSettings.toolExecution">工具执行完成时通知</el-checkbox>
            </el-form-item>
            
            <el-form-item label="系统更新通知">
              <el-checkbox v-model="notificationSettings.systemUpdate">系统更新时通知</el-checkbox>
            </el-form-item>
            
            <el-form-item label="通知持续时间">
              <el-input-number
                v-model="notificationSettings.duration"
                :min="1000"
                :max="10000"
                :step="1000"
                style="width: 200px"
              />
              <span class="unit">毫秒</span>
            </el-form-item>
          </el-form>
        </el-card>
      </el-tab-pane>
      
      <!-- 安全设置 -->
      <el-tab-pane label="安全设置" name="security">
        <el-card class="settings-card">
          <el-form :model="securitySettings" label-width="150px">
            <el-form-item label="会话超时">
              <el-input-number
                v-model="securitySettings.sessionTimeout"
                :min="30"
                :max="1440"
                :step="30"
                style="width: 200px"
              />
              <span class="unit">分钟</span>
            </el-form-item>
            
            <el-form-item label="自动登出">
              <el-checkbox v-model="securitySettings.autoLogout">长时间无操作自动登出</el-checkbox>
            </el-form-item>
            
            <el-form-item label="操作确认">
              <el-checkbox v-model="securitySettings.confirmDangerous">危险操作需要确认</el-checkbox>
            </el-form-item>
            
            <el-form-item label="日志记录">
              <el-checkbox v-model="securitySettings.enableLogging">记录用户操作日志</el-checkbox>
            </el-form-item>
            
            <el-form-item label="IP白名单">
              <el-input 
                v-model="securitySettings.ipWhitelist" 
                type="textarea"
                :rows="3"
                placeholder="每行一个IP地址或IP段，留空表示不限制"
              />
            </el-form-item>
          </el-form>
        </el-card>
      </el-tab-pane>
      
      <!-- 高级设置 -->
      <el-tab-pane label="高级设置" name="advanced">
        <el-card class="settings-card">
          <el-form :model="advancedSettings" label-width="150px">
            <el-form-item label="调试模式">
              <el-checkbox v-model="advancedSettings.debugMode">启用调试模式</el-checkbox>
            </el-form-item>
            
            <el-form-item label="控制台日志">
              <el-checkbox v-model="advancedSettings.consoleLog">显示控制台日志</el-checkbox>
            </el-form-item>
            
            <el-form-item label="性能监控">
              <el-checkbox v-model="advancedSettings.performanceMonitor">启用性能监控</el-checkbox>
            </el-form-item>
            
            <el-form-item label="缓存策略">
              <el-radio-group v-model="advancedSettings.cacheStrategy">
                <el-radio label="aggressive">激进缓存</el-radio>
                <el-radio label="normal">正常缓存</el-radio>
                <el-radio label="minimal">最小缓存</el-radio>
              </el-radio-group>
            </el-form-item>
            
            <el-form-item label="并发请求数">
              <el-input-number
                v-model="advancedSettings.maxConcurrentRequests"
                :min="1"
                :max="20"
                style="width: 200px"
              />
            </el-form-item>
            
            <el-form-item label="重试次数">
              <el-input-number
                v-model="advancedSettings.retryCount"
                :min="0"
                :max="5"
                style="width: 200px"
              />
            </el-form-item>
            
            <el-form-item label="实验性功能">
              <el-checkbox-group v-model="advancedSettings.experimentalFeatures">
                <el-checkbox label="webgl">WebGL加速</el-checkbox>
                <el-checkbox label="worker">Web Worker</el-checkbox>
                <el-checkbox label="pwa">PWA支持</el-checkbox>
              </el-checkbox-group>
            </el-form-item>
          </el-form>
        </el-card>
      </el-tab-pane>
    </el-tabs>
    
    <!-- 系统信息 -->
    <el-card class="info-card">
      <template #header>
        <span>系统信息</span>
      </template>
      
      <el-descriptions :column="2" border>
        <el-descriptions-item label="版本">v0.5.0</el-descriptions-item>
        <el-descriptions-item label="构建时间">2025-07-11</el-descriptions-item>
        <el-descriptions-item label="前端框架">Vue 3.4 + Element Plus</el-descriptions-item>
        <el-descriptions-item label="后端API">MCPStore API v0.5.0</el-descriptions-item>
        <el-descriptions-item label="浏览器">{{ browserInfo }}</el-descriptions-item>
        <el-descriptions-item label="屏幕分辨率">{{ screenResolution }}</el-descriptions-item>
      </el-descriptions>
    </el-card>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { useAppStore } from '@/stores/app'
import { ElMessage } from 'element-plus'

const appStore = useAppStore()

// 响应式数据
const activeTab = ref('basic')
const saving = ref(false)

// 预定义颜色
const predefineColors = [
  '#409EFF',
  '#67C23A',
  '#E6A23C',
  '#F56C6C',
  '#909399',
  '#c71585',
  '#ff8c00',
  '#ffd700'
]

// 设置数据
const basicSettings = ref({
  systemName: 'MCPStore 管理面板',
  apiUrl: 'http://localhost:18200',
  timeout: 30000,
  refreshInterval: 30,
  language: 'zh-CN'
})

const uiSettings = ref({
  theme: 'light',
  primaryColor: '#409EFF',
  sidebarCollapsed: false,
  showBreadcrumb: true,
  enableAnimation: true,
  tableDensity: 'default'
})

const notificationSettings = ref({
  desktop: true,
  sound: false,
  serviceError: true,
  toolExecution: false,
  systemUpdate: true,
  duration: 4500
})

const securitySettings = ref({
  sessionTimeout: 480,
  autoLogout: true,
  confirmDangerous: true,
  enableLogging: true,
  ipWhitelist: ''
})

const advancedSettings = ref({
  debugMode: false,
  consoleLog: false,
  performanceMonitor: true,
  cacheStrategy: 'normal',
  maxConcurrentRequests: 6,
  retryCount: 3,
  experimentalFeatures: []
})

// 计算属性
const browserInfo = computed(() => {
  const ua = navigator.userAgent
  if (ua.includes('Chrome')) return 'Chrome'
  if (ua.includes('Firefox')) return 'Firefox'
  if (ua.includes('Safari')) return 'Safari'
  if (ua.includes('Edge')) return 'Edge'
  return 'Unknown'
})

const screenResolution = computed(() => {
  return `${screen.width} × ${screen.height}`
})

// 方法
const saveSettings = async () => {
  saving.value = true
  try {
    // 保存到localStorage
    const allSettings = {
      basic: basicSettings.value,
      ui: uiSettings.value,
      notification: notificationSettings.value,
      security: securitySettings.value,
      advanced: advancedSettings.value
    }
    
    localStorage.setItem('mcpstore-settings', JSON.stringify(allSettings))
    
    // 应用UI设置
    appStore.setTheme(uiSettings.value.theme)
    appStore.setCollapse(uiSettings.value.sidebarCollapsed)
    
    // 模拟保存延迟
    await new Promise(resolve => setTimeout(resolve, 1000))
    
    ElMessage.success('设置保存成功')
  } catch (error) {
    ElMessage.error('设置保存失败')
  } finally {
    saving.value = false
  }
}

const resetSettings = () => {
  // 重置为默认值
  basicSettings.value = {
    systemName: 'MCPStore 管理面板',
    apiUrl: 'http://localhost:18200',
    timeout: 30000,
    refreshInterval: 30,
    language: 'zh-CN'
  }
  
  uiSettings.value = {
    theme: 'light',
    primaryColor: '#409EFF',
    sidebarCollapsed: false,
    showBreadcrumb: true,
    enableAnimation: true,
    tableDensity: 'default'
  }
  
  notificationSettings.value = {
    desktop: true,
    sound: false,
    serviceError: true,
    toolExecution: false,
    systemUpdate: true,
    duration: 4500
  }
  
  securitySettings.value = {
    sessionTimeout: 480,
    autoLogout: true,
    confirmDangerous: true,
    enableLogging: true,
    ipWhitelist: ''
  }
  
  advancedSettings.value = {
    debugMode: false,
    consoleLog: false,
    performanceMonitor: true,
    cacheStrategy: 'normal',
    maxConcurrentRequests: 6,
    retryCount: 3,
    experimentalFeatures: []
  }
  
  ElMessage.success('设置已重置为默认值')
}

const loadSettings = () => {
  try {
    const saved = localStorage.getItem('mcpstore-settings')
    if (saved) {
      const settings = JSON.parse(saved)
      if (settings.basic) basicSettings.value = { ...basicSettings.value, ...settings.basic }
      if (settings.ui) uiSettings.value = { ...uiSettings.value, ...settings.ui }
      if (settings.notification) notificationSettings.value = { ...notificationSettings.value, ...settings.notification }
      if (settings.security) securitySettings.value = { ...securitySettings.value, ...settings.security }
      if (settings.advanced) advancedSettings.value = { ...advancedSettings.value, ...settings.advanced }
    }
  } catch (error) {
    console.warn('Failed to load settings:', error)
  }
}

// 生命周期
onMounted(() => {
  loadSettings()
})
</script>

<style lang="scss" scoped>
.settings {
  .page-header {
    @include flex-between;
    margin-bottom: 20px;
    
    .header-left {
      .page-title {
        margin: 0 0 4px 0;
        font-size: 24px;
        font-weight: var(--font-weight-medium);
      }
      
      .page-description {
        margin: 0;
        color: var(--text-secondary);
      }
    }
    
    .header-right {
      display: flex;
      gap: 12px;
    }
  }
  
  .settings-tabs {
    margin-bottom: 20px;
    
    .settings-card {
      min-height: 400px;
      
      .unit {
        margin-left: 8px;
        color: var(--text-secondary);
        font-size: var(--font-size-sm);
      }
    }
  }
  
  .info-card {
    margin-top: 20px;
  }
}

// 响应式适配
@include respond-to(xs) {
  .settings {
    .page-header {
      flex-direction: column;
      align-items: flex-start;
      gap: 16px;
      
      .header-right {
        width: 100%;
        justify-content: flex-end;
      }
    }
  }
}
</style>
