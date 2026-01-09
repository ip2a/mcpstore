<template>
  <div class="tabs-container">
    <div 
      v-for="tab in tabs" 
      :key="tab.path"
      class="atom-tab"
      :class="{ 'active': active === tab.path }"
      @click="navigate(tab.path)"
    >
      <span class="tab-title">{{ tab.title }}</span>
      
      <!-- Close Button (Always visible on active, hover on others) -->
      <span 
        v-if="tab.path !== '/system/dashboard'"
        class="tab-close"
        @click.stop="onRemove(tab.path)"
      >
        <el-icon><Close /></el-icon>
      </span>
    </div>
  </div>
</template>

<script setup>
import { computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useTabsStore } from '@/stores/tabs'
import { Close } from '@element-plus/icons-vue'

const route = useRoute()
const router = useRouter()
const tabsStore = useTabsStore()

const active = computed(() => route.path)
const tabs = computed(() => tabsStore.tabs)

watch(
  () => route.fullPath,
  () => {
    if (route.path === '/' || route.meta?.hidden) return
    const title = route.meta?.title || route.name || 'Untitled'
    tabsStore.add({ path: route.path, title })
  },
  { immediate: true }
)

function navigate(path) {
  if (path !== route.path) router.push(path)
}

function onRemove(path) {
  tabsStore.remove(path)
  if (route.path === path) {
    router.push(tabsStore.lastOrHome())
  }
}
</script>

<style lang="scss" scoped>
.tabs-container {
  display: flex;
  align-items: center;
  gap: 4px;
  overflow-x: auto;
  scrollbar-width: none; // Hide scrollbar
  &::-webkit-scrollbar { display: none; }
}

.atom-tab {
  display: flex;
  align-items: center;
  height: 32px;
  padding: 0 10px 0 12px;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.15s ease;
  border: 1px solid transparent;
  max-width: 200px;
  flex-shrink: 0;

  // Default State
  background: transparent;
  color: var(--text-secondary);
  
  &:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
    
    .tab-close { opacity: 1; }
  }

  // Active State
  &.active {
    background: var(--bg-surface);
    border-color: var(--border-color);
    color: var(--text-primary);
    box-shadow: var(--shadow-sm);
    
    .tab-close { opacity: 1; }
  }
}

.tab-title {
  font-size: 13px;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  margin-right: 4px;
}

.tab-close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  border-radius: 4px;
  font-size: 10px;
  opacity: 0; // Hidden by default
  transition: opacity 0.2s, background-color 0.2s;
  
  &:hover {
    background-color: var(--border-color); // Darker hover
    color: var(--color-danger);
  }
}
</style>