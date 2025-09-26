<template>
  <div class="config-page art-full-height">
    <!-- 第一排：横幅和小图表 -->
    <el-row :gutter="20" class="mb-3">
      <el-col :xs="24" :sm="12" :md="12">
        <ArtBasicBanner
          title="配置管理中心"
          :subtitle="headerSubtitle"
          :titleColor="'var(--el-text-color-primary)'"
          :subtitleColor="'var(--el-text-color-secondary)'"
          :backgroundColor="'var(--el-color-info-light-9)'"
          :buttonConfig="{ 
            show: true, 
            text: '刷新配置', 
            color: 'var(--el-color-info)', 
            textColor: '#fff', 
            radius: '6px' 
          }"
          @buttonClick="loadFromFile"
        />
      </el-col>
      <el-col :xs="12" :sm="6" :md="6">
        <div class="card art-custom-card mini-chart-card">
          <div class="chart-header">
            <div class="chart-value">{{ validRate }}%</div>
            <div class="chart-label">配置有效率</div>
          </div>
          <div class="chart-container-mini">
            <ArtRingChart
              :data="validRateChartData"
              :radius="['50%', '80%']"
              :showLegend="false"
              height="120px"
            />
          </div>
        </div>
      </el-col>
      <el-col :xs="12" :sm="6" :md="6">
        <div class="card art-custom-card mini-chart-card">
          <div class="chart-header">
            <div class="chart-value">{{ serviceCards.length }}</div>
            <div class="chart-label">服务配置数</div>
          </div>
          <div class="chart-container-mini">
            <ArtBarChart
              :data="serviceTypeData"
              :xAxisData="serviceTypeLabels"
              :showLegend="false"
              height="120px"
              barWidth="60%"
            />
          </div>
        </div>
      </el-col>
    </el-row>

    <!-- 主体内容区域 -->
    <el-row :gutter="20">
      <!-- 左侧：JSON 编辑器卡片 -->
      <el-col :xs="24" :md="16">
        <el-card class="art-custom-card table-card-enhanced" shadow="hover">
          <template #header>
            <div class="card-header">
              <span>配置文件编辑器</span>
              <el-space>
                <el-tag :type="jsonValid ? 'success' : 'danger'" size="small">{{ jsonValid ? '语法正确' : '语法错误' }}</el-tag>
                <el-tag size="small">字符: {{ jsonText.length }}</el-tag>
              </el-space>
            </div>
          </template>

          <el-input v-model="jsonText" type="textarea" :rows="20" placeholder="MCP JSON配置内容..." />

          <div class="mt-2">
            <el-space>
              <el-button @click="formatJson">格式化</el-button>
              <el-button @click="validateJson">验证</el-button>
              <el-button @click="copyJson">复制</el-button>
              <el-button type="primary" :loading="saving" @click="submitUpdate">提交更新</el-button>
              <el-button type="warning" :loading="loading" @click="loadFromFile">从文件获取最新</el-button>
              <el-button type="info" :loading="loading" @click="resetFile">重置文件</el-button>
            </el-space>
          </div>
        </el-card>
      </el-col>

      <!-- 右侧：文件信息 + 服务预览 -->
      <el-col :xs="24" :md="8">
        <el-card class="art-custom-card info-card-enhanced" shadow="hover">
          <template #header>
            <div class="card-header">
              <span>文件信息</span>
            </div>
          </template>
          <el-descriptions :column="1" size="small" border>
            <el-descriptions-item label="状态">
              <el-tag :type="jsonValid ? 'success' : 'danger'" size="small">{{ jsonValid ? '已校验' : '待修复' }}</el-tag>
            </el-descriptions-item>
            <el-descriptions-item label="格式">JSON</el-descriptions-item>
            <el-descriptions-item label="服务数">{{ serviceCards.length }}</el-descriptions-item>
            <el-descriptions-item label="最后更新">{{ lastUpdated || '-' }}</el-descriptions-item>
          </el-descriptions>
        </el-card>

        <el-card class="art-custom-card service-preview-card mt-3" shadow="hover">
          <template #header>
            <div class="card-header">
              <span>服务预览（{{ serviceCards.length }}）</span>
            </div>
          </template>
          <el-space direction="vertical" fill>
            <el-card v-for="svc in serviceCards" :key="svc.name" class="mini-svc-card svc-clickable" shadow="never" @click="gotoService(svc.name)">
              <template #header>
                <div class="svc-header">
                  <span class="svc-name">{{ svc.name }}</span>
                  <el-tag size="small" :type="svc.type === 'HTTP' ? 'success' : 'info'">{{ svc.type }}</el-tag>
                </div>
              </template>
              <div class="svc-body">
                <div v-if="svc.url">
                  <span class="label">URL:</span> <span class="value">{{ svc.url }}</span>
                </div>
                <div v-if="svc.command">
                  <span class="label">命令:</span> <span class="value">{{ svc.command }}</span>
                </div>
                <div v-if="svc.args?.length">
                  <span class="label">参数:</span> <span class="value">{{ svc.args.join(' ') }}</span>
                </div>
              </div>
            </el-card>
          </el-space>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import { useRouter } from 'vue-router'
import { dashboardApi } from '@/mcp/api/dashboard'

const router = useRouter()
const gotoService = (name: string) => router.push({ path: '/services', query: { q: name } })

const jsonText = ref('')
const jsonValid = ref(true)
const lastUpdated = ref('')
const loading = ref(false)
const saving = ref(false)

const parseMcpFromResponse = (res: any) => {
  // 后端示例返回格式中 data = { mcpServers: { ... } }
  return res?.data || {}
}

// 计算属性
const headerSubtitle = computed(() => {
  const total = serviceCards.value.length
  const httpCount = serviceCards.value.filter(s => s.type === 'HTTP').length
  const cmdCount = total - httpCount
  return `管理 ${total} 个服务配置，${httpCount} 个HTTP服务，${cmdCount} 个命令服务`
})

const validRate = computed(() => {
  return jsonValid.value ? 100 : 0
})

const validRateChartData = computed(() => [
  { value: validRate.value, name: '有效' },
  { value: 100 - validRate.value, name: '无效' }
])

const serviceTypeData = computed(() => {
  const httpCount = serviceCards.value.filter(s => s.type === 'HTTP').length
  const cmdCount = serviceCards.value.length - httpCount
  return [httpCount, cmdCount]
})

const serviceTypeLabels = computed(() => ['HTTP', 'Command'])

const serviceCards = computed(() => {
  try {
    const parsed = JSON.parse(jsonText.value || '{}')
    const servers = parsed.mcpServers || {}
    return Object.keys(servers).map((name) => {
      const cfg: any = (servers as any)[name] || {}
      const type = cfg.url ? 'HTTP' : 'Command'
      return { name, type, url: cfg.url, command: cfg.command, args: cfg.args }
    })
  } catch (e) {
    return []
  }
})

const formatJson = () => {
  try {
    const obj = JSON.parse(jsonText.value || '{}')
    jsonText.value = JSON.stringify(obj, null, 2)
    ElMessage.success('已格式化')
  } catch {
    ElMessage.error('JSON 格式错误')
  }
}

const validateJson = () => {
  try {
    const obj = JSON.parse(jsonText.value || '{}')
    if (!obj || typeof obj !== 'object') throw new Error('必须是对象')
    if (!obj.mcpServers || typeof obj.mcpServers !== 'object') throw new Error('缺少 mcpServers 对象')
    jsonValid.value = true
    ElMessage.success('语法正确')
  } catch (e: any) {
    jsonValid.value = false
    ElMessage.error(`校验失败: ${e?.message || 'JSON 错误'}`)
  }
}

const copyJson = async () => {
  try {
    await navigator.clipboard.writeText(jsonText.value)
    ElMessage.success('已复制')
  } catch {
    ElMessage.error('复制失败')
  }
}

const loadFromFile = async () => {
  try {
    loading.value = true
    const res = await dashboardApi.showMcpConfig()
    const data = parseMcpFromResponse(res)
    jsonText.value = JSON.stringify(data, null, 2)
    lastUpdated.value = new Date().toLocaleString()

    validateJson()
    ElMessage.success('已从文件获取最新')
  } catch (e) {
    ElMessage.error('获取失败')
  } finally {
    loading.value = false
  }
}

const resetFile = async () => {
  try {
    loading.value = true
    await dashboardApi.resetMcpJsonFile()
    await loadFromFile()
    ElMessage.success('已重置')
  } catch (e) {
    ElMessage.error('重置失败')
  } finally {
    loading.value = false
  }
}

const submitUpdate = async () => {
  try {
    saving.value = true
    const obj = JSON.parse(jsonText.value || '{}')
    // 采用 update_config 接口：整体更新，client_id_or_service_name 用 all
    const res = await dashboardApi.updateConfig('all', obj)
    if (res?.success) {
      ElMessage.success('已更新配置')
    } else {
      ElMessage.warning(res?.message || '更新完成但未返回 success=true')
    }
  } catch (e: any) {
    ElMessage.error(`更新失败: ${e?.message || 'JSON 错误'}`)
  } finally {
    saving.value = false
  }
}

onMounted(() => {
  loadFromFile()
})
</script>

<style scoped lang="scss">
.config-page {
  padding: 20px;

  .mb-3 {
    margin-bottom: 20px;
  }

  // 卡片基础样式
  .card,
  .art-custom-card {
    background: var(--el-bg-color);
    border: 1px solid var(--el-border-color-light);
    border-radius: 8px;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
    padding: 20px;
    margin-bottom: 20px;
    transition: all 0.3s ease;

    &:hover {
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    }

    .box-title {
      font-size: 18px;
      font-weight: 500;
      color: var(--el-text-color-primary);
      margin-bottom: 16px;
    }

    .subtitle {
      color: var(--el-text-color-secondary);
      font-size: 14px;
    }

    .text-success { color: var(--el-color-success); }
    .text-warning { color: var(--el-color-warning); }
    .text-info { color: var(--el-color-info); }
    .text-danger { color: var(--el-color-danger); }
  }

  // 小图表卡片样式
  .mini-chart-card {
    .chart-header {
      margin-bottom: 12px;

      .chart-value {
        font-size: 24px;
        font-weight: 600;
        color: var(--el-text-color-primary);
        margin-bottom: 4px;
      }

      .chart-label {
        font-size: 14px;
        color: var(--el-text-color-secondary);
      }
    }

    .chart-container-mini {
      height: 120px;
    }
  }

  // 增强表格卡片样式
  .table-card-enhanced {
    background: var(--el-bg-color);
    border: 1px solid var(--el-border-color-light);
    border-radius: 12px;
    box-shadow: 0 4px 6px rgba(0, 0, 0, 0.07);
    overflow: hidden;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);

    &:hover {
      box-shadow: 0 8px 20px rgba(0, 0, 0, 0.12);
      transform: translateY(-1px);
      border-color: var(--el-color-info-light-7);
    }

    // JSON编辑器样式优化
    :deep(.el-textarea__inner) {
      font-family: var(--el-font-family-mono, 'Consolas', 'Monaco', 'Courier New', monospace);
      font-size: 13px;
      line-height: 1.5;
      background: var(--el-fill-color-extra-light);
      border: 1px solid var(--el-border-color-lighter);
      border-radius: 6px;
      
      &:focus {
        border-color: var(--el-color-info);
      }
    }
  }

  // 信息卡片增强样式
  .info-card-enhanced {
    border: 1px solid var(--el-border-color-light);
    border-radius: 12px;
    box-shadow: 0 4px 6px rgba(0, 0, 0, 0.07);
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);

    &:hover {
      box-shadow: 0 8px 20px rgba(0, 0, 0, 0.12);
      transform: translateY(-1px);
      border-color: var(--el-color-info-light-7);
    }

    :deep(.el-descriptions) {
      .el-descriptions__label {
        font-weight: 500;
        color: var(--el-text-color-primary);
      }
      
      .el-descriptions__content {
        color: var(--el-text-color-regular);
      }
    }
  }

  // 服务预览卡片样式
  .service-preview-card {
    border: 1px solid var(--el-border-color-light);
    border-radius: 12px;
    box-shadow: 0 4px 6px rgba(0, 0, 0, 0.07);
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);

    &:hover {
      box-shadow: 0 8px 20px rgba(0, 0, 0, 0.12);
      transform: translateY(-1px);
      border-color: var(--el-color-info-light-7);
    }
  }

  // 卡片头部样式
  .card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-weight: 500;
    color: var(--el-text-color-primary);
  }

  // 服务卡片样式
  .mini-svc-card {
    margin-bottom: 8px;
    border: 1px solid var(--el-border-color-lighter);
    border-radius: 6px;
    transition: all 0.2s ease;

    &:hover {
      border-color: var(--el-border-color);
      box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
    }

    &.svc-clickable {
      cursor: pointer;
      
      &:hover {
        border-color: var(--el-color-info-light-5);
        background: var(--el-color-info-light-9);
      }
    }

    :deep(.el-card__header) {
      padding: 12px 16px;
      border-bottom: 1px solid var(--el-border-color-lighter);
    }

    :deep(.el-card__body) {
      padding: 12px 16px;
    }

    .svc-header {
      display: flex;
      align-items: center;
      justify-content: space-between;

      .svc-name {
        font-weight: 600;
        color: var(--el-text-color-primary);
      }
    }

    .svc-body {
      .label {
        color: var(--el-text-color-secondary);
        margin-right: 6px;
        font-weight: 500;
      }

      .value {
        color: var(--el-text-color-primary);
        font-family: var(--el-font-family-mono, monospace);
        font-size: 12px;
        word-break: break-all;
      }

      > div {
        margin-bottom: 4px;
        line-height: 1.4;

        &:last-child {
          margin-bottom: 0;
        }
      }
    }
  }

  // 按钮组样式
  .mt-2 {
    margin-top: 16px;
    padding-top: 16px;
    border-top: 1px solid var(--el-border-color-lighter);
  }

  .mt-3 {
    margin-top: 20px;
  }

  // 标签样式优化
  :deep(.el-tag) {
    font-weight: 500;
    border-radius: 4px;
  }

  // 按钮样式优化
  :deep(.el-button) {
    border-radius: 6px;
    font-weight: 500;
    transition: all 0.2s ease;

    &.el-button--primary {
      box-shadow: 0 2px 4px rgba(64, 158, 255, 0.3);

      &:hover {
        box-shadow: 0 4px 8px rgba(64, 158, 255, 0.4);
      }
    }
  }

  // 响应式设计
  @media (max-width: 768px) {
    padding: 12px;

    .card,
    .art-custom-card {
      padding: 16px;
      margin-bottom: 16px;
    }

    .mini-chart-card {
      .chart-header {
        margin-bottom: 8px;

        .chart-value {
          font-size: 20px;
        }

        .chart-label {
          font-size: 12px;
        }
      }

      .chart-container-mini {
        height: 100px;
      }
    }

    .mini-svc-card {
      :deep(.el-card__header) {
        padding: 10px 12px;
      }

      :deep(.el-card__body) {
        padding: 10px 12px;
      }

      .svc-body {
        .label {
          font-size: 11px;
        }

        .value {
          font-size: 11px;
        }
      }
    }

    .mt-2 {
      margin-top: 12px;
      padding-top: 12px;
    }

    // 按钮组响应式
    :deep(.el-space) {
      flex-wrap: wrap;
    }

    :deep(.el-button) {
      font-size: 12px;
      padding: 6px 12px;
    }
  }
}
</style>

