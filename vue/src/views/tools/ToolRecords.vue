<template>
  <div class="tool-records">
    <div class="page-header">
      <div class="header-left">
        <h2 class="page-title">工具使用记录</h2>
        <p class="page-description">查看最近的工具调用记录，支持按工具和服务筛选</p>
      </div>
      <div class="header-right">
        <el-input v-model="filters.toolName" placeholder="按工具名称筛选" clearable class="w-220" @change="reload" />
        <el-input v-model="filters.serviceName" placeholder="按服务名称筛选" clearable class="w-220" @change="reload" />
        <el-button type="primary" :loading="loading" @click="reload">
          <el-icon><Refresh /></el-icon>
          刷新
        </el-button>
      </div>
    </div>

    <el-card shadow="never">
      <el-table :data="records" stripe style="width: 100%" :loading="loading">
        <el-table-column type="index" label="#" width="60" />
        <el-table-column label="时间" width="180">
          <template #default="{ row }">
            <div class="time-col">
              <div>{{ formatDateTime(normalizeTs(row.timestamp || row.created_at)) }}</div>
              <div class="sub">{{ formatRelativeTime(normalizeTs(row.timestamp || row.created_at)) }}</div>
            </div>
          </template>
        </el-table-column>
        <el-table-column prop="tool_name" label="工具" width="220" />
        <el-table-column prop="service_name" label="服务" width="200" />
        <el-table-column label="状态" width="100">
          <template #default="{ row }">
            <el-tag :type="row.error ? 'danger' : 'success'" size="small">
              {{ row.error ? '失败' : '成功' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="耗时(ms)" width="110" align="right">
          <template #default="{ row }">{{ row.elapsed_ms ?? row.duration_ms ?? row.response_time ?? '-' }}</template>
        </el-table-column>
        <el-table-column label="请求参数" min-width="260">
          <template #default="{ row }">
            <el-tooltip :content="formatJSON(row.params || row.args || row.arguments)" placement="top" :show-after="400">
              <span class="mono">{{ truncateText(formatJSON(row.params || row.args || row.arguments), 80) }}</span>
            </el-tooltip>
          </template>
        </el-table-column>
        <el-table-column label="响应摘要" min-width="260">
          <template #default="{ row }">
            <el-tooltip :content="formatJSON(row.result || row.response)" placement="top" :show-after="400">
              <span class="mono">{{ truncateText(formatJSON(row.result || row.response), 80) }}</span>
            </el-tooltip>
          </template>
        </el-table-column>
      </el-table>

      <div class="table-footer">
        <el-pagination
          background
          layout="total, sizes, prev, pager, next, jumper"
          :total="total"
          v-model:page-size="pageSize"
          v-model:current-page="page"
          :page-sizes="[10, 20, 50, 100]"
          @current-change="reload"
          @size-change="handleSizeChange"
        />
      </div>
    </el-card>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { api } from '@/api'
import { formatDateTime, formatRelativeTime, formatJSON, truncateText } from '@/utils/format'
import { Refresh } from '@element-plus/icons-vue'

const records = ref([])
const loading = ref(false)
const page = ref(1)
const pageSize = ref(20)
const total = ref(0)

const filters = ref({ toolName: '', serviceName: '' })

const reload = async () => {
  loading.value = true
  try {
    const params = {
      tool_name: filters.value.toolName || undefined,
      service_name: filters.value.serviceName || undefined,
      page: page.value,
      page_size: pageSize.value
    }
    const data = await api.store.getToolRecordsPaged(params)
    // 兼容返回结构：{ executions: [], summary: {...}, pagination: {...} } 或 { list: [], total: N }
    const list = data?.executions || data?.list || []
    const totalCount = data?.pagination?.total || data?.total || list.length

    records.value = list
    total.value = totalCount
  } finally {
    loading.value = false
  }
}

const handleSizeChange = () => {
  page.value = 1
  reload()
}

onMounted(() => reload())

// 工具方法：将秒级/字符串时间戳规范化为毫秒数
const normalizeTs = (ts) => {
  if (!ts) return ''
  if (typeof ts === 'number') return ts < 1e12 ? ts * 1000 : ts
  const n = Number(ts)
  if (!isNaN(n)) return n < 1e12 ? n * 1000 : n
  return ts
}
</script>

<style scoped lang="scss">
.tool-records {
  width: 92%;
  margin: 0 auto;
  max-width: none;
  .page-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;

    .page-title { margin: 0 0 4px; font-size: 22px; font-weight: 600; }
    .page-description { margin: 0; color: var(--el-text-color-secondary); }

    .header-right { display: flex; gap: 12px; }
    .w-220 { width: 220px; }
  }

  .time-col { .sub { color: var(--el-text-color-secondary); font-size: 12px; } }
  .mono { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace; }

  .table-footer { display: flex; justify-content: flex-end; padding: 12px 0; }
}
</style>

