import { apiRequest } from './request'

// Base path for global store cache operations
const BASE_PATH = '/for_store/cache'

export const cacheApi = {
  /**
   * Get cache statistics and configuration
   * @returns {Promise}
   */
  inspect: () => apiRequest.get(`${BASE_PATH}/inspect`),

  /**
   * Get entity cache items
   * @param {Object} params - Query parameters
   * @param {string} [params.type] - Comma separated types (e.g. 'services,tools')
   * @param {string} [params.key] - Filter by specific key
   * @returns {Promise}
   */
  getEntities: (params = {}) => apiRequest.get(`${BASE_PATH}/entities`, { params }),

  /**
   * Get relation cache items
   * @param {Object} params - Query parameters
   * @param {string} [params.type] - Comma separated types
   * @param {string} [params.key] - Filter by specific key
   * @returns {Promise}
   */
  getRelations: (params = {}) => apiRequest.get(`${BASE_PATH}/relations`, { params }),

  /**
   * Get state cache items
   * @param {Object} params - Query parameters
   * @param {string} [params.type] - Comma separated types
   * @param {string} [params.key] - Filter by specific key
   * @returns {Promise}
   */
  getStates: (params = {}) => apiRequest.get(`${BASE_PATH}/states`, { params }),

  /**
   * Dump full cache snapshot
   * @returns {Promise}
   */
  dump: () => apiRequest.get(`${BASE_PATH}/dump`)
}
