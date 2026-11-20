import type { HostsPayload, SettingsPayload, UiLanguage, UpdateSettingsPayload } from "@/lib/api"
import { type HostsConfig, parseHostsPayload, serializeHostsPayload } from "@/lib/api/hosts"
import {
  parseServiceTabsPrefs,
  type ServiceTabSort,
  type ServiceTabsPrefs,
} from "@/features/mcp-registry/service-tabs-prefs"
import type { I18nKey } from "@/lib/i18n-core"

export type SectionId =
  | "general"
  | "connection"
  | "diagnostics"
  | "advanced"
  | "about"

export type HostDraft = {
  name: string
  urls: string[]
}

export type DiagnosticsDraft = {
  runtime_log: {
    enabled: boolean
    level: string
    max_size_bytes: number
    retention_days: number | null
  }
  requests: {
    enabled: boolean
    capture_tool: boolean
    capture_management: boolean
    capture_cli: boolean
    capture_payload: boolean
    max_payload_bytes: number
    retention_days: number | null
    max_records: number | null
  }
  gateway_calls: {
    enabled: boolean
    persist: boolean
    memory_capacity: number
    retention_days: number | null
    max_calls: number | null
  }
  frames: {
    enabled: boolean
    capture_gateway: boolean
    capture_upstream: boolean
    capture_payload: boolean
    max_payload_bytes: number
    retention_days: number | null
    max_records: number | null
  }
  event_history: {
    enabled: boolean
    capacity: number
  }
  stdio_stderr: {
    enabled: boolean
    tail_lines: number
    forward_to_runtime_log: boolean
    include_in_error: boolean
  }
  daemon_output: {
    enabled: boolean
  }
}

const defaultDiagnostics: DiagnosticsDraft = {
  runtime_log: {
    enabled: true,
    level: "info",
    max_size_bytes: 5 * 1024 * 1024,
    retention_days: null,
  },
  requests: {
    enabled: true,
    capture_tool: true,
    capture_management: true,
    capture_cli: true,
    capture_payload: true,
    max_payload_bytes: 1024 * 1024,
    retention_days: 30,
    max_records: null,
  },
  gateway_calls: {
    enabled: true,
    persist: true,
    memory_capacity: 256,
    retention_days: 30,
    max_calls: 5000,
  },
  frames: {
    enabled: true,
    capture_gateway: true,
    capture_upstream: true,
    capture_payload: true,
    max_payload_bytes: 1024 * 1024,
    retention_days: 7,
    max_records: null,
  },
  event_history: { enabled: true, capacity: 10_000 },
  stdio_stderr: {
    enabled: true,
    tail_lines: 50,
    forward_to_runtime_log: true,
    include_in_error: true,
  },
  daemon_output: { enabled: true },
}

export type SettingsDraft = {
  language: UiLanguage
  hosts: HostsConfig
  api: {
    port: number
  }
  web: {
    port: number
    auth: boolean
  }
  diagnostics: DiagnosticsDraft
  serviceTabs: ServiceTabsPrefs
}

export const sections: Array<{ id: SectionId; labelKey: I18nKey }> = [
  { id: "general", labelKey: "general" },
  { id: "connection", labelKey: "connection" },
  { id: "diagnostics", labelKey: "diagnostics" },
  { id: "advanced", labelKey: "advanced" },
  { id: "about", labelKey: "about" },
]

export function hostsToList(hosts: HostsConfig): HostDraft[] {
  return Object.entries(hosts.entries).map(([name, entry]) => ({
    name,
    urls: entry.urls,
  }))
}

function withDefaults<T extends object>(defaults: T, patch?: Partial<T>): T {
  return { ...defaults, ...patch }
}

export function settingsDraft(settings: SettingsPayload): SettingsDraft {
  if (!settings.hosts || !settings.api || !settings.web) {
    throw new Error("Settings payload is missing required sections")
  }

  const hosts = parseHostsPayload(settings.hosts)
  const diagnostics = settings.diagnostics

  return {
    language: settings.language ?? "auto",
    hosts,
    api: {
      port: settings.api.port!,
    },
    web: {
      port: settings.web.port!,
      auth: settings.web.auth === true,
    },
    diagnostics: {
      runtime_log: withDefaults(defaultDiagnostics.runtime_log, diagnostics?.runtime_log),
      requests: withDefaults(defaultDiagnostics.requests, diagnostics?.requests),
      gateway_calls: withDefaults(defaultDiagnostics.gateway_calls, diagnostics?.gateway_calls),
      frames: withDefaults(defaultDiagnostics.frames, diagnostics?.frames),
      event_history: withDefaults(defaultDiagnostics.event_history, diagnostics?.event_history),
      stdio_stderr: withDefaults(defaultDiagnostics.stdio_stderr, diagnostics?.stdio_stderr),
      daemon_output: withDefaults(defaultDiagnostics.daemon_output, diagnostics?.daemon_output),
    },
    serviceTabs: parseServiceTabsPrefs(settings.service_tabs),
  }
}

export function payloadFromDraft(draft: SettingsDraft): UpdateSettingsPayload {
  return {
    language: draft.language,
    api: draft.api,
    web: draft.web,
    hosts: serializeHostsPayload(draft.hosts) as HostsPayload,
    diagnostics: draft.diagnostics,
    service_tabs: {
      sort: draft.serviceTabs.sort,
      show_disconnected: draft.serviceTabs.showDisconnected,
    },
  }
}

export type { ServiceTabSort, ServiceTabsPrefs }
