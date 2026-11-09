import { api, appApi, buildQuery } from "./client"
import type {
  AddServiceInput,
  AgentItem,
  AuthFlow,
  AuthOperationResult,
  AuthStatus,
  AuthStatusView,
  AuthorizationStart,
  CacheBackend,
  CacheReport,
  ConfigFilePayload,
  ConfigReport,
  ConfigRevision,
  DesiredState,
  DiagnosticsSettingsPayload,
  FailureInfo,
  FailurePhase,
  HealthState,
  LogSettingsPayload,
  McpServerCapabilities,
  McpServerMetadata,
  MetaPayload,
  PromptInfo,
  ReadinessReason,
  ReadinessStatus,
  RecoveryState,
  ResourceInfo,
  ResourceTemplateInfo,
  RuntimePhase,
  ScopeDescriptor,
  ScopeRef,
  ServiceAddress,
  ServiceAuthState,
  ServiceInstance,
  ServiceLifecycleConfig,
  ServiceRestartPolicy,
  ServiceStartupPolicy,
  ServiceState,
  SettingsPathsPayload,
  SettingsPayload,
  ToolAvailability,
  ToolInfo,
  ToolVisibilityFilter,
  ToolsStatus,
  UiLanguage,
  UpdateServiceScopeInput,
  UpdateSettingsPayload,
} from "../api"

export type HostStatus = {
  name: string
  online: boolean
  url: string | null
  node_id: string | null
  version: string | null
  store: string | null
  latency_ms: number | null
  error: string | null
}

export function getHostStatus(name: string): Promise<HostStatus> {
  return appApi<HostStatus>(`/v1/hosts/${encodeURIComponent(name)}/status`)
}

export async function showConfig(
  options: { format?: string; service?: ServiceAddress } = {},
): Promise<ConfigReport> {
  const format = options.format ?? "native"
  const service = options.service
  return api(
    `/config${buildQuery({
      format: format === "native" ? undefined : format,
      service_name: service?.service_name,
      scope: service?.scope.type === "agent" ? "agent" : "store",
      agent_id: service?.scope.type === "agent" ? service.scope.agent_id : undefined,
    })}`,
  )
}

export async function showAgentConfig(
  agentId: string,
  options: { format?: string; service?: ServiceAddress } = {},
): Promise<ConfigReport> {
  const format = options.format ?? "native"
  return api(
    `/scopes/agents/${encodeURIComponent(agentId)}/config${buildQuery({
      format: format === "native" ? undefined : format,
      service_name: options.service?.service_name,
    })}`,
  )
}

export async function getMeta(): Promise<MetaPayload> {
  return appApi<MetaPayload>("/v1/meta")
}

export async function updateSettings(payload: UpdateSettingsPayload): Promise<SettingsPayload> {
  return appApi<SettingsPayload>("/v1/settings", {
    method: "PUT",
    body: JSON.stringify(payload),
  })
}

export async function resetConfig() {
  return api("/config/reset", { method: "POST" })
}

export async function resetAgentConfig(agentId: string) {
  return api(`/scopes/agents/${encodeURIComponent(agentId)}/reset`, {
    method: "POST",
  })
}
