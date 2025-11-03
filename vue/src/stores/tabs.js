import { defineStore } from 'pinia'

export const useTabsStore = defineStore('tabs', {
  state: () => ({
    tabs: [
      { path: '/dashboard', title: '仪表板' }
    ]
  }),
  actions: {
    add(tab) {
      if (!tab || !tab.path) return
      if (!this.tabs.find(t => t.path === tab.path)) {
        this.tabs.push({
          path: tab.path,
          title: tab.title || tab.name || '未命名'
        })
      }
    },
    remove(path) {
      this.tabs = this.tabs.filter(t => t.path !== path)
    },
    lastOrHome() {
      return this.tabs[this.tabs.length - 1]?.path || '/dashboard'
    }
  }
})

