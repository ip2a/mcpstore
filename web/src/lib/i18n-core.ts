import type { UiLanguage } from "@/lib/api"

export const dictionaries = {
  zh: {
    about: "关于",
    auto: "自动",
    cancel: "取消",
    chooseLanguage: "控制设置界面的语言偏好；后端可按需使用该值。",
    configFile: "配置文件",
    configReadonlyDescription: "只读展示后端 meta 接口返回的配置文件内容。",
    configFileMissing: "未返回配置文件",
    defaultBackupDir: "默认备份目录",
    general: "通用",
    generalDescription: "这些字段会通过 /api/v1/settings 保存到后端。",
    language: "语言",
    loadingSettings: "正在加载设置",
    logFilePathMissing: "日志路径会在后端 meta 接口完成后显示。",
    logMaxSizeMb: "日志大小上限 MB",
    logRetentionDays: "日志保留天数",
    metaApi: "Meta API",
    retry: "重试",
    save: "保存",
    saved: "设置已保存",
    saveFailed: "设置保存失败",
    settings: "设置",
    settingsDescription: "管理 mcpstore Web 设置和配置文件信息。",
    settingsUnavailable: "设置服务暂不可用",
    settingsNav: "设置",
    settingsApi: "Settings API",
    unlimited: "留空表示不限制。",
    backupDirMissing: "后端未返回解析后的目录。",
    version: "版本",
  },
  en: {
    about: "About",
    auto: "Auto",
    cancel: "Cancel",
    chooseLanguage: "Choose the settings UI language; the backend may also use this value.",
    configFile: "Config File",
    configReadonlyDescription: "Read-only config content returned by the backend meta API.",
    configFileMissing: "No config file returned",
    defaultBackupDir: "Default Backup Directory",
    general: "General",
    generalDescription: "These fields are saved through /api/v1/settings.",
    language: "Language",
    loadingSettings: "Loading settings",
    logFilePathMissing: "The log path will appear after the backend meta API returns it.",
    logMaxSizeMb: "Log Size Limit MB",
    logRetentionDays: "Log Retention Days",
    metaApi: "Meta API",
    retry: "Retry",
    save: "Save",
    saved: "Settings saved",
    saveFailed: "Failed to save settings",
    settings: "Settings",
    settingsDescription: "Manage mcpstore Web settings and config file information.",
    settingsUnavailable: "Settings service is unavailable",
    settingsNav: "Settings",
    settingsApi: "Settings API",
    unlimited: "Leave empty for unlimited.",
    backupDirMissing: "The backend did not return a resolved directory.",
    version: "Version",
  },
} as const

export type I18nKey = keyof typeof dictionaries.zh
export type ResolvedLanguage = keyof typeof dictionaries

export function browserLanguage(): ResolvedLanguage {
  if (typeof navigator !== "undefined" && navigator.language.toLowerCase().startsWith("zh")) return "zh"
  return "en"
}

export function resolveLanguage(language: UiLanguage | undefined): ResolvedLanguage {
  if (language === "zh" || language === "en") return language
  return browserLanguage()
}

export function translate(language: ResolvedLanguage, key: I18nKey, vars?: Record<string, string | number | null | undefined>) {
  let text: string = dictionaries[language][key] || dictionaries.zh[key] || key
  if (!vars) return text
  for (const [name, value] of Object.entries(vars)) {
    text = text.replaceAll(`{${name}}`, String(value ?? ""))
  }
  return text
}
