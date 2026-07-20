import { api, buildQuery, request } from "./client";
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
} from "../api";

export async function showConfig(
  options: { format?: string; instanceId?: string } = {},
): Promise<ConfigReport> {
  const format = options.format ?? "native";
  return request(
    `/config${buildQuery({ format: format === "native" ? undefined : format, instance_id: options.instanceId })}`,
  );
}

export async function showAgentConfig(
  agentId: string,
  options: { format?: string; instanceId?: string } = {},
): Promise<ConfigReport> {
  const format = options.format ?? "native";
  return request(
    `/scopes/agents/${encodeURIComponent(agentId)}/config${buildQuery({ format: format === "native" ? undefined : format, instance_id: options.instanceId })}`,
  );
}

export async function getMeta(): Promise<MetaPayload> {
  return api<MetaPayload>("/v1/meta");
}

export async function updateSettings(
  payload: UpdateSettingsPayload,
): Promise<SettingsPayload> {
  return api<SettingsPayload>("/v1/settings", {
    method: "PUT",
    body: JSON.stringify(payload),
  });
}

export async function resetConfig() {
  return request("/config/reset", { method: "POST" });
}

export async function resetAgentConfig(agentId: string) {
  return request(`/scopes/agents/${encodeURIComponent(agentId)}/reset`, {
    method: "POST",
  });
}
