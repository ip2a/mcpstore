import { API_ENDPOINTS } from './config'
import { formatApiPath, extractResponseData } from './utils'
import { apiRequest } from './request'

/**
 * 数据空间管理 API 服务
 * 对应 MCPStore API v1.0.0 的数据空间管理端点
 */
export const dataSpaceApi = {
  /**
   * 数据空间信息
   */
  getDataSpaceInfo: () => apiRequest.get(API_ENDPOINTS.DATA_SPACE.INFO)
    .then(res => extractResponseData(res.data)),
  
  /**
   * 工作空间管理
   */
  listWorkspaces: () => apiRequest.get(API_ENDPOINTS.DATA_SPACE.WORKSPACE_LIST)
    .then(res => extractResponseData(res.data, [])),
  
  createWorkspace: (data) => apiRequest.post(API_ENDPOINTS.DATA_SPACE.WORKSPACE_CREATE, data),
  
  switchWorkspace: (data) => apiRequest.post(API_ENDPOINTS.DATA_SPACE.WORKSPACE_SWITCH, data),
  
  getCurrentWorkspace: () => apiRequest.get(API_ENDPOINTS.DATA_SPACE.WORKSPACE_CURRENT)
    .then(res => extractResponseData(res.data)),
  
  deleteWorkspace: (workspaceName) => apiRequest.delete(
    formatApiPath(API_ENDPOINTS.DATA_SPACE.WORKSPACE_DELETE, { workspace_name: workspaceName })
  )
}