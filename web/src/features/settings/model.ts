import type { SettingsPayload, UiLanguage, UpdateSettingsPayload } from "@/lib/api"
import type { I18nKey } from "@/lib/i18n-core"

export type SectionId = "general" | "backend" | "diagnostics" | "config" | "about"

export type SettingsDraft = {
  language: UiLanguage
  default_backup_dir: string
  server: {
    port: number
    web_port: number
  }
  diagnostics: {
    enabled: boolean
    runtime_enabled: boolean
    runtime_max_size_bytes: number
    runtime_retention_days: number | null
    history_enabled: boolean
    storage: "memory" | "disk"
    max_records: number
    max_size_bytes: number
    retention_days: number | null
    payload: "none" | "metadata" | "full"
  }
}

export const sections: Array<{ id: SectionId; labelKey: I18nKey }> = [
  { id: "general", labelKey: "general" },
  { id: "backend", labelKey: "coreBackend" },
  { id: "diagnostics", labelKey: "diagnostics" },
  { id: "config", labelKey: "configFile" },
  { id: "about", labelKey: "about" },
]

export function settingsDraft(settings?: SettingsPayload): SettingsDraft {
  return {
    language: settings?.language || "auto",
    default_backup_dir: typeof settings?.default_backup_dir === "string" ? settings.default_backup_dir : "./backups",
    server: {
      port: settings?.server?.port || 1820,
      web_port: settings?.server?.web_port || 1828,
    },
    diagnostics: {
      enabled: settings?.diagnostics?.enabled !== false,
      runtime_enabled: settings?.diagnostics?.runtime_log?.enabled === true,
      runtime_max_size_bytes: settings?.diagnostics?.runtime_log?.max_size_bytes || 5 * 1024 * 1024,
      runtime_retention_days: typeof settings?.diagnostics?.runtime_log?.retention_days === "number" ? settings.diagnostics.runtime_log.retention_days : null,
      history_enabled: settings?.diagnostics?.history?.enabled === true,
      storage: settings?.diagnostics?.history?.storage === "disk" ? "disk" : "memory",
      max_records: settings?.diagnostics?.history?.max_records || 10000,
      max_size_bytes: settings?.diagnostics?.history?.max_size_bytes || 50 * 1024 * 1024,
      retention_days: typeof settings?.diagnostics?.history?.retention_days === "number" ? settings.diagnostics.history.retention_days : null,
      payload: settings?.diagnostics?.history?.payload || "metadata",
    },
  }
}

export function payloadFromDraft(draft: SettingsDraft): UpdateSettingsPayload {
  return {
    language: draft.language,
    default_backup_dir: draft.default_backup_dir || "./backups",
    server: draft.server,
    diagnostics: {
      enabled: draft.diagnostics.enabled,
      runtime_log: {
        enabled: draft.diagnostics.runtime_enabled,
        max_size_bytes: draft.diagnostics.runtime_max_size_bytes,
        retention_days: draft.diagnostics.runtime_retention_days,
      },
      history: {
        enabled: draft.diagnostics.history_enabled,
        storage: draft.diagnostics.storage,
        max_records: draft.diagnostics.max_records,
        max_size_bytes: draft.diagnostics.max_size_bytes,
        retention_days: draft.diagnostics.retention_days,
        payload: draft.diagnostics.payload,
      },
    },
  }
}
