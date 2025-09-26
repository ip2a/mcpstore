import { defineStore } from 'pinia'
import { mcpApi } from '../api'

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
        const arr = await mcpApi.listServices()
        this.services = Array.isArray(arr) ? arr : []
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
        const arr = await mcpApi.listTools()
        this.tools = Array.isArray(arr) ? arr : []
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

