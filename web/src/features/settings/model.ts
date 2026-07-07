import type { SettingsPayload, UiLanguage, UpdateSettingsPayload } from "@/lib/api"
import type { I18nKey } from "@/lib/i18n-core"

export type SectionId = "general" | "config" | "about"

export type SettingsDraft = {
  language: UiLanguage
  default_backup_dir: string
  logging: {
    max_size_bytes: number | null
    retention_days: number | null
  }
}

export const sections: Array<{ id: SectionId; labelKey: I18nKey }> = [
  { id: "general", labelKey: "general" },
  { id: "config", labelKey: "configFile" },
  { id: "about", labelKey: "about" },
]

export function settingsDraft(settings?: SettingsPayload): SettingsDraft {
  return {
    language: settings?.language || "auto",
    default_backup_dir: typeof settings?.default_backup_dir === "string" ? settings.default_backup_dir : "./backups",
    logging: {
      max_size_bytes: typeof settings?.logging?.max_size_bytes === "number" ? settings.logging.max_size_bytes : 5 * 1024 * 1024,
      retention_days: typeof settings?.logging?.retention_days === "number" ? settings.logging.retention_days : null,
    },
  }
}

export function logSizeMb(draft: SettingsDraft) {
  const bytes = Number(draft.logging.max_size_bytes || 0)
  return String((bytes > 0 ? bytes : 5 * 1024 * 1024) / 1024 / 1024).replace(/\.0$/, "")
}

export function payloadFromDraft(draft: SettingsDraft): UpdateSettingsPayload {
  return {
    language: draft.language,
    default_backup_dir: draft.default_backup_dir || "./backups",
    logging: {
      max_size_bytes: draft.logging.max_size_bytes,
      retention_days: draft.logging.retention_days,
    },
  }
}
