import type { App } from 'vue'
import { setupHighlightDirective } from './highlight'
import { setupRippleDirective } from './ripple'
// import { setupRolesDirective } from './roles'

export function setupGlobDirectives(app: App) {
  // 移除权限相关指令，仅保留通用指令
  setupHighlightDirective(app) // 高亮指令
  setupRippleDirective(app) // 水波纹指令
}
