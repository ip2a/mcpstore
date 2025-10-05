<template>
  <div class="tools-exec-page art-full-height">
    <!-- 顶部横幅与两个小图表：完全复用 /tools 第一行的风格 -->
    <el-row :gutter="20" class="mb-3">
      <!-- 左侧横幅：仪表盘/模板标准组件 -->
      <el-col :xs="24" :sm="12" :md="12">
        <ArtBasicBanner
          title="工具使用概览"
          :subtitle="headerSubtitle"
          :backgroundColor="'var(--el-color-primary-light-9)'"
          :buttonConfig="{ 
            show: true, 
            text: '刷新数据', 
            color: 'var(--el-color-primary)', 
            textColor: '#fff', 
            radius: '6px' 
          }"
          @buttonClick="refreshAll"
        />
      </el-col>

      <!-- 右侧：两张小图表卡，采用 template/charts 的卡片结构（card art-custom-card + card-header + 图表组件） -->
      <el-col :xs="12" :sm="6" :md="6">
        <div class="card art-custom-card">
          <div class="card-header">
            <span>{{ healthyCount }}/{{ totalServicesCount }} 健康服务</span>
          </div>
          <ArtRingChart :data="healthyDonutData" :radius="['50%', '80%']" :showLegend="false" height="120px" />
        </div>
      </el-col>
      <el-col :xs="12" :sm="6" :md="6">
        <div class="card art-custom-card">
          <div class="card-header">
            <span>{{ totalServicesCount }} 服务总数</span>
          </div>
          <ArtBarChart :data="toolsByServiceData" :xAxisData="toolsByServiceLabels" :showLegend="false" height="120px" barWidth="60%" />
        </div>
      </el-col>
    </el-row>

    <!-- 主体：去步骤化，改为四卡片布局 -->
    <el-row :gutter="16">
      <!-- 选择服务 -->
      <el-col :xs="24" :md="12">
        <div class="card art-custom-card">
          <ArtTableHeader :loading="false" @refresh="refreshAll" :showSearchBar="true" @update:showSearchBar="() => {}">
            <template #left>
              <h3 class="table-title">选择服务</h3>
            </template>
          </ArtTableHeader>
          <ArtSearchBar
            v-model="serviceForm"
            :items="serviceItems"
            :span="12"
            :showExpand="false"
          />
        </div>
      </el-col>

      <!-- 选择工具 -->
      <el-col :xs="24" :md="12">
        <div class="card art-custom-card">
          <ArtTableHeader :loading="false" @refresh="refreshAll" :showSearchBar="true" @update:showSearchBar="() => {}">
            <template #left>
              <h3 class="table-title">选择工具</h3>
            </template>
          </ArtTableHeader>
          <ArtSearchBar
            v-model="toolForm"
            :items="toolItems"
            :span="12"
            :showExpand="false"
          />
        </div>
      </el-col>

      <!-- 参数与执行 -->
      <el-col :xs="24" :md="12">
        <div class="card art-custom-card">
          <ArtTableHeader :loading="false">
            <template #left>
              <h3 class="table-title">参数与执行</h3>
            </template>
            <template #right>
              <el-button text type="warning" @click="resetFlow">重置</el-button>
            </template>
          </ArtTableHeader>

          <el-descriptions :column="1" border class="mb-3" v-if="selectedTool">
            <el-descriptions-item label="服务">{{ selectedService }}</el-descriptions-item>
            <el-descriptions-item label="工具">{{ selectedTool?.name }}</el-descriptions-item>
            <el-descriptions-item label="描述">{{ selectedTool?.description }}</el-descriptions-item>
          </el-descriptions>

          <el-empty v-if="!selectedTool" description="请先选择服务和工具" />

          <div v-else>
            <el-form ref="formRef" :model="formModel" :rules="formRules" label-width="120px" status-icon>
              <div v-for="field in formFields" :key="field.key">
                <el-form-item :prop="field.key" :label="field.label" :required="field.required">
                  <template v-if="field.type === 'string' && !field.enum">
                    <el-input v-model="formModel[field.key]" :placeholder="field.placeholder" clearable />
                  </template>
                  <template v-else-if="field.type === 'string' && field.enum">
                    <el-select v-model="formModel[field.key]" filterable clearable :placeholder="field.placeholder">
                      <el-option v-for="opt in field.enum" :key="opt" :label="opt" :value="opt" />
                    </el-select>
                  </template>
                  <template v-else-if="field.type === 'number' || field.type === 'integer'">
                    <el-input-number
                      v-model="formModel[field.key]"
                      :min="field.minimum ?? Number.NEGATIVE_INFINITY"
                      :max="field.maximum ?? Number.POSITIVE_INFINITY"
                      :step="field.type === 'integer' ? 1 : 0.1"
                    />
                  </template>
                  <template v-else-if="field.type === 'boolean'">
                    <el-switch v-model="formModel[field.key]" />
                  </template>
                  <template v-else-if="field.type === 'array' && field.itemsType === 'string'">
                    <el-select v-model="formModel[field.key]" multiple filterable allow-create default-first-option>
                      <el-option v-for="opt in (field.enum || [])" :key="opt" :label="opt" :value="opt" />
                    </el-select>
                  </template>
                  <template v-else>
                    <el-input v-model="formModel[field.key]" :placeholder="field.placeholder" clearable />
                  </template>
                  <template #label>
                    <span>{{ field.label }}</span>
                    <el-tooltip v-if="field.description" effect="dark" :content="field.description" placement="top">
                      <i class="iconfont-sys" style="margin-left:6px;color:var(--el-text-color-secondary)">&#xe61c;</i>
                    </el-tooltip>
                  </template>
                </el-form-item>
              </div>

              <el-form-item>
                <el-button type="primary" :loading="execLoading" :disabled="!selectedTool" @click="onExecute">执行</el-button>
              </el-form-item>
            </el-form>
          </div>
        </div>
      </el-col>

      <!-- 执行结果 -->
      <el-col :xs="24" :md="12">
        <div class="card art-custom-card">
          <ArtTableHeader :loading="false">
            <template #left>
              <h3 class="table-title">执行结果</h3>
            </template>
          </ArtTableHeader>
          <div v-if="execResultText" class="exec-result">{{ execResultText }}</div>
          <el-empty v-else description="暂无结果" />
        </div>
      </el-col>
    </el-row>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted, watch } from 'vue'
// 全局已注册（见 types/components.d.ts），这里无需显式导入本地组件
import { ElMessage } from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import { mcpApi } from '../../api/index'

defineOptions({ name: 'ToolExecutor' })

// 顶部横幅统计文案：与 /tools 一致的口径
const tools = ref<any[]>([])
const services = ref<any[]>([])
const headerSubtitle = computed(() => {
  const svcCount = services.value.length
  const toolCount = tools.value.length
  return `当前共有 ${toolCount} 个工具，来源于 ${svcCount} 个服务。`
})

// 复用风格：成功率与按服务分布（基于真实数据计算）
const successRate = computed(() => {
  if (services.value.length === 0) return 100
  const healthyCount = services.value.filter((s: any) => s.status === 'healthy').length
  return Math.round((healthyCount / services.value.length) * 100)
})
const successChartData = computed(() => [
  { value: successRate.value, name: '成功' },
  { value: 100 - successRate.value, name: '失败' }
])
const totalTools = computed(() => tools.value.length)
const toolsByServiceMap = computed(() => {
  const map: Record<string, number> = {}
  tools.value.forEach((t) => {
    const sn = t.service || t.service_name || 'unknown'
    map[sn] = (map[sn] || 0) + 1
  })
  return map
})
const toolsByServiceLabels = computed(() => Object.keys(toolsByServiceMap.value))
const toolsByServiceData = computed(() => Object.values(toolsByServiceMap.value))



// 搜索条：选择服务/工具 使用内置 ArtSearchBar，而不是手写样式
const serviceForm = reactive({ service: '' })
const serviceItems = [
  { key: 'service', label: '服务', type: 'select', props: { placeholder: '请选择服务', options: [], filterable: true, clearable: true, style: 'width:100%' } }
]
const toolForm = reactive({ tool: '' })
const toolItems = [
  { key: 'tool', label: '工具', type: 'select', props: { placeholder: '请选择工具', options: [], filterable: true, clearable: true, style: 'width:100%' } }
]


// 第一步：服务选择（仅健康）
const selectedService = ref<string>('')
const healthyServices = computed(() => {
  const list = services.value || []
  return list.filter((s: any) => s.status === 'healthy' || s.status === 'initializing')
})

// 第二步：工具选择（按服务过滤）
const selectedToolName = ref<string>('')
const filteredTools = computed(() => {
  if (!selectedService.value) return []
  return tools.value.filter((t) => (t.service || t.service_name) === selectedService.value)
})
const selectedTool = computed(() => filteredTools.value.find((t) => t.name === selectedToolName.value))

const healthyCount = computed(() => services.value.filter((s: any) => s.status === 'healthy').length)
const totalServicesCount = computed(() => services.value.length)
const healthyDonutData = computed(() => [
  { name: 'Healthy', value: healthyCount.value },
  { name: 'Others', value: Math.max(totalServicesCount.value - healthyCount.value, 0) }
])

const successDonutData = computed(() => [
  { name: 'Success', value: successRate.value },
  { name: 'Fail', value: 100 - successRate.value }
])

// 第三步：参数动态表单
const formRef = ref<FormInstance>()
const formModel = reactive<Record<string, any>>({})
const formRules = ref<FormRules>({})

// 基于 JSON Schema 的简易字段映射
interface FieldDef {
  key: string
  label: string
  type: string
  enum?: string[]
  itemsType?: string
  required?: boolean
  placeholder?: string
  minimum?: number
  maximum?: number
  description?: string
}
const formFields = ref<FieldDef[]>([])

function buildFormFromSchema(schema: any) {
  formFields.value = []
  Object.keys(formModel).forEach((k) => delete formModel[k])
  formRules.value = {}

  if (!schema || schema.type !== 'object' || !schema.properties) return

  const required: string[] = schema.required || []
  const properties = schema.properties || {}

  Object.entries<any>(properties).forEach(([key, prop]) => {
    const field: FieldDef = {
      key,
      label: prop.title || key,
      type: prop.type || 'string',
      required: required.includes(key),
      placeholder: prop.description || ''
    }

    if (Array.isArray(prop.enum)) field.enum = prop.enum
    if (typeof prop.minimum === 'number') field.minimum = prop.minimum
    if (typeof prop.maximum === 'number') field.maximum = prop.maximum
    if (prop.type === 'array' && prop.items) field.itemsType = prop.items.type
    if (prop.description) field.description = prop.description

    formFields.value.push(field)

    // 初始值：default 或类型默认
    if (prop.default !== undefined) {
      formModel[key] = prop.default
    } else if (prop.type === 'array') {
      formModel[key] = []
    } else if (prop.type === 'boolean') {
      formModel[key] = false
    } else {
      formModel[key] = ''
    }

    // 规则：必填
    if (field.required) {
      formRules.value[key] = [{ required: true, message: `${field.label}为必填项`, trigger: 'blur' }]
    }
  })
}

function prepareParamsAndNext() {
  const schema = selectedTool.value?.input_schema || selectedTool.value?.inputSchema
  buildFormFromSchema(schema)
}

// 执行：接入旧版API契约 /for_store/call_tool
const execLoading = ref(false)
const execResultText = ref<string>('')
async function onExecute() {
  if (!formRef.value) return
  try {
    await formRef.value.validate()
  } catch {
    ElMessage.warning('请完善参数后再执行')
    return
  }

  execLoading.value = true
  execResultText.value = ''

  try {
    const toolName = selectedTool.value?.name as string
    const args = { ...formModel }

    // 改为 { tool_name, args }，兼容后端
    const res = await mcpApi.callTool(toolName, args)
    execResultText.value = JSON.stringify(res, null, 2)
    if ((res as any)?.success) ElMessage.success('工具执行成功')
  } catch (e: any) {
    ElMessage.error(e?.message || '工具执行失败')
    execResultText.value = e?.message || String(e)
  } finally {
    execLoading.value = false
  }
}

function resetFlow() {
  selectedService.value = ''
  selectedToolName.value = ''
  execResultText.value = ''
  formFields.value = []
  Object.keys(formModel).forEach((k) => delete formModel[k])
}

// 刷新：同时刷新服务与工具
async function refreshAll() {
  await Promise.all([loadServices(), loadTools()])
  ElMessage.success('数据已刷新')
}

async function loadServices() {
  try {
    const arr = await mcpApi.listServices()
    services.value = Array.isArray(arr) ? arr : []
  } catch (e) {
    console.error(e)
    ElMessage.error('获取服务失败')
  }
}

async function loadTools() {
  try {
    const arr = await mcpApi.listTools()
    tools.value = Array.isArray(arr) ? arr : []
  } catch (e) {
    console.error(e)
    ElMessage.error('获取工具失败')
  }
}

// 选择联动（在依赖都已声明之后再设置 watcher，避免未初始化问题）
watch(services, () => {
  const options = healthyServices.value.map((svc: any) => ({ label: svc.name, value: svc.name }))
  const svcItem = serviceItems.find((i) => i.key === 'service') as any
  if (svcItem) svcItem.props.options = options
})

watch([selectedService, filteredTools], () => {
  const options = filteredTools.value.map((t: any) => ({ label: t.name, value: t.name }))
  const toolItem = toolItems.find((i) => i.key === 'tool') as any
  if (toolItem) toolItem.props.options = options
})

watch(
  () => serviceForm.service,
  (val) => {
    selectedService.value = val
    selectedToolName.value = ''
  }
)

watch(
  () => toolForm.tool,
  (val) => {
    selectedToolName.value = val
    if (val) prepareParamsAndNext()
  }
)

onMounted(async () => {
  await refreshAll()
})
</script>

<style scoped lang="scss">
.tools-exec-page {
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
    .card-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 16px;
      
      span {
        font-size: 14px;
        font-weight: 500;
        color: var(--el-text-color-primary);
      }
    }

    .chart-header {
      display: flex;
      justify-content: space-between;
      align-items: baseline;
      margin-bottom: 12px;

      .chart-value {
        font-size: 22px;
        font-weight: 600;
        color: var(--el-text-color-primary);
        margin-bottom: 4px;
      }

      .chart-label {
        font-size: 14px;
        color: var(--el-text-color-secondary);
      }
    }
  }

  // 表格标题样式
  .table-title {
    font-size: 16px;
    font-weight: 600;
    color: var(--el-text-color-primary);
    margin: 0;
  }

  // 执行结果样式优化
  .exec-result {
    background: var(--el-fill-color-extra-light);
    border: 1px solid var(--el-border-color-lighter);
    border-radius: 6px;
    padding: 16px;
    font-family: var(--el-font-family-mono, 'Consolas', 'Monaco', 'Courier New', monospace);
    font-size: 13px;
    line-height: 1.5;
    color: var(--el-text-color-primary);
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 400px;
    overflow-y: auto;

    &::-webkit-scrollbar {
      width: 6px;
    }

    &::-webkit-scrollbar-track {
      background: var(--el-fill-color-light);
      border-radius: 3px;
    }

    &::-webkit-scrollbar-thumb {
      background: var(--el-border-color);
      border-radius: 3px;

      &:hover {
        background: var(--el-border-color-dark);
      }
    }
  }

  // 表单样式优化
  :deep(.el-form) {
    .el-form-item {
      margin-bottom: 18px;

      .el-form-item__label {
        font-weight: 500;
        color: var(--el-text-color-primary);
      }
    }

    .el-descriptions {
      margin-bottom: 20px;
      
      :deep(.el-descriptions__label) {
        font-weight: 500;
        color: var(--el-text-color-primary);
      }
    }
  }

  .section {
    .section-title {
      font-size: 16px;
      font-weight: 600;
      margin-bottom: 12px;
      color: var(--el-text-color-primary);
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
      .card-header span {
        font-size: 13px;
      }
    }

    .table-title {
      font-size: 15px;
    }

    .exec-result {
      padding: 12px;
      font-size: 12px;
      max-height: 300px;
    }
  }
}
</style>

