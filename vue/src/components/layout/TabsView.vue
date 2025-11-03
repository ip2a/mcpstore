<template>
  <div class="tabs-view">
    <el-tabs
      v-model="active"
      type="card"
      size="small"
      @tab-remove="onRemove"
      class="tabs-card"
    >
      <el-tab-pane
        v-for="tab in tabs"
        :key="tab.path"
        :name="tab.path"
        :label="tab.title"
        :closable="tab.path !== '/dashboard'"
      />
    </el-tabs>
  </div>
</template>

<script setup>
import { computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useTabsStore } from '@/stores/tabs'

const route = useRoute()
const router = useRouter()
const tabsStore = useTabsStore()

// 当前激活的标签与当前路由同步
const active = computed({
  get: () => route.path,
  set: (path) => { if (path && path !== route.path) router.push(path) }
})

// 对外展示的标签列表
const tabs = computed(() => tabsStore.tabs)

// 路由变化时自动加入标签
watch(
  () => route.fullPath,
  () => {
    // 跳过根路径或被隐藏的路由，避免出现“未命名”标签
    if (route.path === '/' || route.meta?.hidden) return
    const title = route.meta?.title || route.name || '未命名'
    tabsStore.add({ path: route.path, title })
  },
  { immediate: true }
)

function onRemove(name) {
  tabsStore.remove(name)
  if (route.path === name) {
    router.push(tabsStore.lastOrHome())
  }
}
</script>

<style scoped>
.tabs-view { background: transparent; }
.tabs-card { padding: 0 12px; }

/* Make card tabs cleaner: remove separators, unify subtle radius */
:deep(.el-tabs--card > .el-tabs__header) {
  border-bottom: 0; /* we already draw a border on the container */
}
:deep(.el-tabs--card > .el-tabs__header .el-tabs__nav) {
  border: 0;
}
:deep(.el-tabs--card > .el-tabs__header .el-tabs__item) {
  border: 0;
  border-radius: var(--el-border-radius-small);
  margin-right: 6px; /* spacing between tabs instead of borders */
  background: var(--el-bg-color);
}
:deep(.el-tabs--card > .el-tabs__header .el-tabs__item.is-active) {
  background: var(--el-color-white);
  border: 1px solid var(--el-border-color);
}
:deep(.el-tabs--card > .el-tabs__header .el-tabs__item:first-child.is-active) {
  border-left-color: transparent; /* 减少左侧割裂感 */
}
:deep(.el-tabs--card > .el-tabs__header .el-tabs__item:last-child) {
  margin-right: 0;
}
</style>

