import type { HostsPayload, SettingsPayload, UiLanguage, UpdateSettingsPayload } from "@/lib/api"
import {
  type HostsConfig,
  isDefaultHostsOnly,
  migrateHostsFromLegacyStorage,
  parseHostsPayload,
  serializeHostsPayload,
} from "@/lib/api/hosts"
import type { I18nKey } from "@/lib/i18n-core"

export type SectionId = "overview" | "general" | "connection" | "diagnostics" | "config" | "about"

export type HostDraft = {
  name: string
  url: string
}

export type SettingsDraft = {
  language: UiLanguage
  hosts: HostsConfig
  api: {
    port: number
  }
  web: {
    port: number
  }
  diagnostics: {
    enabled: boolean
    runtime_enabled: boolean
    runtime_max_size_bytes: number
    runtime_retention_days: number | null
  }
}

export const sections: Array<{ id: SectionId; labelKey: I18nKey }> = [
  { id: "overview", labelKey: "overview" },
  { id: "general", labelKey: "general" },
  { id: "connection", labelKey: "connection" },
  { id: "diagnostics", labelKey: "diagnostics" },
  { id: "config", labelKey: "configFile" },
  { id: "about", labelKey: "about" },
]

export function hostsToList(hosts: HostsConfig): HostDraft[] {
  return Object.entries(hosts.entries).map(([name, entry]) => ({
    name,
    url: entry.url,
  }))
}

export function settingsDraft(settings?: SettingsPayload): SettingsDraft {
  let hosts = parseHostsPayload(settings?.hosts)
  if (isDefaultHostsOnly(hosts)) {
    const migrated = migrateHostsFromLegacyStorage()
    if (migrated) hosts = migrated
  }

  return {
    language: settings?.language || "auto",
    hosts,
    api: {
      port: settings?.api?.port || 1820,
    },
    web: {
      port: settings?.web?.port || 1828,
    },
    diagnostics: {
      enabled: settings?.diagnostics?.enabled !== false,
      runtime_enabled: settings?.diagnostics?.runtime_log?.enabled === true,
      runtime_max_size_bytes: settings?.diagnostics?.runtime_log?.max_size_bytes || 5 * 1024 * 1024,
      runtime_retention_days: typeof settings?.diagnostics?.runtime_log?.retention_days === "number" ? settings.diagnostics.runtime_log.retention_days : null,
    },
  }
}

export function payloadFromDraft(draft: SettingsDraft): UpdateSettingsPayload {
  return {
    language: draft.language,
    api: draft.api,
    web: draft.web,
    hosts: serializeHostsPayload(draft.hosts) as HostsPayload,
    diagnostics: {
      enabled: draft.diagnostics.enabled,
      runtime_log: {
        enabled: draft.diagnostics.runtime_enabled,
        max_size_bytes: draft.diagnostics.runtime_max_size_bytes,
        retention_days: draft.diagnostics.runtime_retention_days,
      },
    },
  }
}
