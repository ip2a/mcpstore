<template>
  <div class="mcp-page">
    <el-card shadow="hover">
      <div class="header">
        <div class="title">服务列表</div>
        <div class="actions">
          <el-button :loading="loading" type="primary" @click="refresh">刷新</el-button>
        </div>
      </div>
      <el-table :data="services" v-loading="loading" border>
        <el-table-column label="#" type="index" width="60"/>
        <el-table-column prop="name" label="服务名称"/>
      </el-table>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { onMounted, computed } from 'vue'
import { useMcpSystemStore } from '../store/system'

const store = useMcpSystemStore()
const services = computed(() => store.services.map(name => ({ name })))
const loading = computed(() => store.loading)

function refresh() {
  store.fetchServices()
}

onMounted(() => {
  store.fetchServices()
})
</script>

<style scoped>
.mcp-page { padding: 12px; }
.header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; }
.title { font-weight: 600; }
</style>

