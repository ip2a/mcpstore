// 导出所有 API 模块
export * from './config'
export * from './utils'
export * from './request'
export * from './store'
export * from './agent'
export * from './monitoring'
export * from './dataSpace'
export * from './langChain'

// 便捷的统一导出
import { storeApi } from './store'
import { agentApi } from './agent'
import { monitoringApi } from './monitoring'
import { dataSpaceApi } from './dataSpace'
import { langChainApi } from './langChain'

export const api = {
  store: storeApi,
  agent: agentApi,
  monitoring: monitoringApi,
  dataSpace: dataSpaceApi,
  langChain: langChainApi
}