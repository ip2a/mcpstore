<template>
  <div class="performance-chart">
    <div class="chart-header">
      <h3 class="chart-title">
        {{ title }}
      </h3>
      <div class="chart-controls">
        <el-select
          v-model="timeRange"
          size="small"
          @change="handleTimeRangeChange"
        >
          <el-option
            label="最近1小时"
            value="1h"
          />
          <el-option
            label="最近6小时"
            value="6h"
          />
          <el-option
            label="最近24小时"
            value="24h"
          />
          <el-option
            label="最近7天"
            value="7d"
          />
        </el-select>
        <el-button
          size="small"
          :icon="Refresh"
          :loading="loading"
          @click="refreshData"
        >
          刷新
        </el-button>
      </div>
    </div>
    
    <div
      ref="chartRef"
      class="chart-container"
    />
    
    <div
      v-if="loading"
      class="chart-loading"
    >
      <el-icon class="is-loading">
        <Loading />
      </el-icon>
      <span>加载中...</span>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue'
import * as echarts from 'echarts'
import { Refresh, Loading } from '@element-plus/icons-vue'

const props = defineProps({
  title: {
    type: String,
    default: '性能监控'
  },
  data: {
    type: Array,
    default: () => []
  },
  metrics: {
    type: Array,
    default: () => ['cpu', 'memory']
  },
  height: {
    type: String,
    default: '300px'
  }
})

const emit = defineEmits(['refresh', 'timeRangeChange'])

// 响应式数据
const chartRef = ref(null)
const chart = ref(null)
const timeRange = ref('1h')
const loading = ref(false)

// 图表配置
const getChartOption = () => {
  const colors = ['#409EFF', '#67C23A', '#E6A23C', '#F56C6C']
  
  return {
    tooltip: {
      trigger: 'axis',
      axisPointer: {
        type: 'cross',
        label: {
          backgroundColor: '#6a7985'
        }
      },
      formatter: (params) => {
        let result = `${params[0].axisValue}<br/>`
        params.forEach((param, index) => {
          const unit = param.seriesName.includes('CPU') || param.seriesName.includes('内存') ? '%' : ''
          result += `${param.marker}${param.seriesName}: ${param.value}${unit}<br/>`
        })
        return result
      }
    },
    legend: {
      data: props.metrics.map(metric => getMetricName(metric)),
      top: 10
    },
    grid: {
      left: '3%',
      right: '4%',
      bottom: '3%',
      top: '15%',
      containLabel: true
    },
    xAxis: {
      type: 'category',
      boundaryGap: false,
      data: props.data.map(item => formatTime(item.timestamp))
    },
    yAxis: {
      type: 'value',
      axisLabel: {
        formatter: '{value}%'
      }
    },
    series: props.metrics.map((metric, index) => ({
      name: getMetricName(metric),
      type: 'line',
      smooth: true,
      symbol: 'circle',
      symbolSize: 6,
      lineStyle: {
        width: 2
      },
      areaStyle: {
        opacity: 0.1
      },
      color: colors[index % colors.length],
      data: props.data.map(item => item[metric] || 0)
    }))
  }
}

// 获取指标名称
const getMetricName = (metric) => {
  const names = {
    cpu: 'CPU使用率',
    memory: '内存使用率',
    disk: '磁盘使用率',
    network: '网络IO'
  }
  return names[metric] || metric
}

// 格式化时间
const formatTime = (timestamp) => {
  const date = new Date(timestamp)
  return date.toLocaleTimeString('zh-CN', { 
    hour: '2-digit', 
    minute: '2-digit' 
  })
}

// 初始化图表
const initChart = () => {
  if (!chartRef.value) return
  
  chart.value = echarts.init(chartRef.value)
  updateChart()
  
  // 监听窗口大小变化
  window.addEventListener('resize', handleResize)
}

// 更新图表
const updateChart = () => {
  if (!chart.value) return
  
  const option = getChartOption()
  chart.value.setOption(option, true)
}

// 处理窗口大小变化
const handleResize = () => {
  if (chart.value) {
    chart.value.resize()
  }
}

// 刷新数据
const refreshData = async () => {
  loading.value = true
  try {
    emit('refresh')
  } finally {
    loading.value = false
  }
}

// 处理时间范围变化
const handleTimeRangeChange = (value) => {
  emit('timeRangeChange', value)
}

// 监听数据变化
watch(() => props.data, () => {
  nextTick(() => {
    updateChart()
  })
}, { deep: true })

// 监听指标变化
watch(() => props.metrics, () => {
  nextTick(() => {
    updateChart()
  })
}, { deep: true })

// 生命周期
onMounted(() => {
  nextTick(() => {
    initChart()
  })
})

onUnmounted(() => {
  if (chart.value) {
    chart.value.dispose()
  }
  window.removeEventListener('resize', handleResize)
})
</script>

<style lang="scss" scoped>
.performance-chart {
  .chart-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
    
    .chart-title {
      margin: 0;
      font-size: 16px;
      font-weight: var(--font-weight-medium);
      color: var(--el-text-color-primary);
    }
    
    .chart-controls {
      display: flex;
      gap: 8px;
      align-items: center;
    }
  }
  
  .chart-container {
    width: 100%;
    height: v-bind(height);
    position: relative;
  }
  
  .chart-loading {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    color: var(--el-text-color-secondary);
    
    .el-icon {
      font-size: 24px;
    }
  }
}
</style>
