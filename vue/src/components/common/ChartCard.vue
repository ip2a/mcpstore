<template>
  <div
    :class="['chart-card', cardStyleClass, { 'chart-card-loading': loading }]"
  >
    <!-- 卡片头部 -->
    <div v-if="title || $slots.header" class="chart-card-header">
      <slot name="header">
        <div class="chart-card-title-wrapper">
          <h3 class="chart-card-title">{{ title }}</h3>
          <p v-if="subtitle" class="chart-card-subtitle">{{ subtitle }}</p>
        </div>
      </slot>

      <!-- 操作按钮区域 -->
      <div v-if="$slots.actions" class="chart-card-actions">
        <slot name="actions"></slot>
      </div>
    </div>

    <!-- 图表内容区域 -->
    <div class="chart-card-body" :style="bodyStyle">
      <!-- 加载状态 -->
      <div v-if="loading" class="chart-card-loader">
        <div class="shimmer" style="height: 100%; border-radius: 8px;"></div>
      </div>

      <!-- 空状态 -->
      <div v-else-if="empty" class="chart-card-empty">
        <slot name="empty">
          <el-empty :description="emptyText" :image-size="80" />
        </slot>
      </div>

      <!-- 图表容器 -->
      <div
        v-else
        ref="chartRef"
        class="chart-container"
        :style="{ height: height }"
      ></div>
    </div>

    <!-- 卡片底部 -->
    <div v-if="$slots.footer" class="chart-card-footer">
      <slot name="footer"></slot>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted, watch, computed, nextTick } from 'vue'
import * as echarts from 'echarts'

const props = defineProps({
  // 标题
  title: {
    type: String,
    default: ''
  },
  // 副标题
  subtitle: {
    type: String,
    default: ''
  },
  // 图表配置（ECharts option）
  option: {
    type: Object,
    required: true
  },
  // 图表高度
  height: {
    type: String,
    default: '300px'
  },
  // 卡片样式: glass | plain | borderless
  cardStyle: {
    type: String,
    default: 'glass',
    validator: (value) => ['glass', 'plain', 'borderless'].includes(value)
  },
  // 加载状态
  loading: {
    type: Boolean,
    default: false
  },
  // 空状态
  empty: {
    type: Boolean,
    default: false
  },
  // 空状态文本
  emptyText: {
    type: String,
    default: '暂无数据'
  },
  // 自适应容器大小
  autoResize: {
    type: Boolean,
    default: true
  },
  // 主题（light | dark）
  theme: {
    type: String,
    default: null
  }
})

const emit = defineEmits(['chart-ready', 'chart-click'])

const chartRef = ref(null)
const chartInstance = ref(null)

// 卡片样式类
const cardStyleClass = computed(() => {
  switch (props.cardStyle) {
    case 'glass':
      return 'glass-card'
    case 'plain':
      return 'chart-card-plain'
    case 'borderless':
      return 'chart-card-borderless'
    default:
      return ''
  }
})

// Body 样式
const bodyStyle = computed(() => {
  return {
    minHeight: props.height
  }
})

// 初始化图表
const initChart = async () => {
  if (!chartRef.value || props.loading || props.empty) return

  await nextTick()

  // 销毁已存在的实例
  if (chartInstance.value) {
    chartInstance.value.dispose()
  }

  // 创建新实例
  const theme = props.theme || (document.documentElement.classList.contains('dark') ? 'dark' : null)
  chartInstance.value = echarts.init(chartRef.value, theme)

  // 设置配置
  chartInstance.value.setOption(props.option, true)

  // 监听点击事件
  chartInstance.value.on('click', (params) => {
    emit('chart-click', params)
  })

  // 发出 ready 事件
  emit('chart-ready', chartInstance.value)
}

// 更新图表
const updateChart = () => {
  if (chartInstance.value && props.option) {
    chartInstance.value.setOption(props.option, true)
  }
}

// 调整大小
const resizeChart = () => {
  if (chartInstance.value) {
    chartInstance.value.resize()
  }
}

// 监听配置变化
watch(
  () => props.option,
  () => {
    updateChart()
  },
  { deep: true }
)

// 监听加载和空状态变化
watch(
  () => [props.loading, props.empty],
  () => {
    if (!props.loading && !props.empty) {
      nextTick(() => {
        initChart()
      })
    }
  }
)

// 窗口大小变化监听
let resizeObserver = null

onMounted(() => {
  initChart()

  // 自适应容器大小
  if (props.autoResize) {
    window.addEventListener('resize', resizeChart)

    // 使用 ResizeObserver 监听容器大小变化
    if (chartRef.value && typeof ResizeObserver !== 'undefined') {
      resizeObserver = new ResizeObserver(() => {
        resizeChart()
      })
      resizeObserver.observe(chartRef.value)
    }
  }
})

onUnmounted(() => {
  // 清理
  if (chartInstance.value) {
    chartInstance.value.dispose()
    chartInstance.value = null
  }

  if (props.autoResize) {
    window.removeEventListener('resize', resizeChart)
    if (resizeObserver) {
      resizeObserver.disconnect()
    }
  }
})

// 暴露方法给父组件
defineExpose({
  chartInstance,
  resize: resizeChart,
  refresh: initChart
})
</script>

<style lang="scss" scoped>
.chart-card {
  border-radius: var(--border-radius-xl);
  overflow: hidden;
  transition: var(--transition-base);

  &-plain {
    background: var(--bg-color);
    border: 1px solid var(--border-lighter);
    box-shadow: var(--shadow-sm);

    &:hover {
      box-shadow: var(--shadow-md);
    }
  }

  &-borderless {
    background: transparent;
  }

  &-loading {
    pointer-events: none;
  }

  &-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    padding: var(--spacing-20) var(--spacing-20) var(--spacing-12);
    border-bottom: 1px solid var(--border-extra-light);
  }

  &-title-wrapper {
    flex: 1;
  }

  &-title {
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
    color: var(--text-primary);
    margin: 0;
  }

  &-subtitle {
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
    margin: var(--spacing-4) 0 0;
  }

  &-actions {
    flex-shrink: 0;
    margin-left: var(--spacing-16);
  }

  &-body {
    position: relative;
    padding: var(--spacing-16);
  }

  &-loader {
    position: absolute;
    inset: var(--spacing-16);
    border-radius: var(--border-radius-md);
  }

  &-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 200px;
    color: var(--text-secondary);
  }

  &-footer {
    padding: var(--spacing-12) var(--spacing-20) var(--spacing-16);
    border-top: 1px solid var(--border-extra-light);
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
  }
}

.chart-container {
  width: 100%;
  transition: var(--transition-base);
}

// 玻璃态卡片特殊样式
.glass-card {
  .chart-card-header {
    border-bottom-color: rgba(255, 255, 255, 0.1);
  }

  .chart-card-footer {
    border-top-color: rgba(255, 255, 255, 0.1);
  }
}

// 响应式设计
@media (max-width: 768px) {
  .chart-card {
    &-header {
      flex-direction: column;
      gap: var(--spacing-12);
    }

    &-actions {
      width: 100%;
      margin-left: 0;
    }

    &-body {
      padding: var(--spacing-12);
    }
  }

  .chart-container {
    height: 250px !important;
  }
}

// 暗色模式适配
:root.dark {
  .chart-card-plain {
    background: var(--bg-color-secondary);
    border-color: var(--border-base);
  }

  .chart-card-header,
  .chart-card-footer {
    border-color: var(--border-base);
  }
}
</style>
