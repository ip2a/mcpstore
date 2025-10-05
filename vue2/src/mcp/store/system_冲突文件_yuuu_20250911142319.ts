import { defineStore } from 'pinia'
import { dashboardApi } from '../api/dashboard'

export const useMcpSystemStore = defineStore('mcp-system', {
  state: () => ({
    services: [] as string[],
    tools: [] as any[],
    loading: false,
    lastUpdate: null as Date | null,
    error: null as string | null
  }),
  actions: {
    async fetchServices() {
      if (this.loading) return
      this.loading = true
      this.error = null
      try {
        const res = await dashboardApi.getServices()
        const list = res?.data?.services || []
        // 该 store 仅需要名称数组以供简单列表演示
        this.services = list.map((s: any) => s.name)
        this.lastUpdate = new Date()
      } catch (e: any) {
        this.error = e?.message || '加载服务失败'
        this.services = []
      } finally {
        this.loading = false
      }
    },
    async fetchTools() {
      if (this.loading) return
      this.loading = true
      this.error = null
      try {
        const res = await dashboardApi.getTools()
        const list = res?.data || []
        this.tools = Array.isArray(list) ? list : []
        this.lastUpdate = new Date()
      } catch (e: any) {
        this.error = e?.message || '加载工具失败'
        this.tools = []
      } finally {
        this.loading = false
      }
    }
  }
})

