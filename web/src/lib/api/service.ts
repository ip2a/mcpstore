import { api, apiUrl, buildQuery, readJson, request } from "./client";
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

export async function health() {
  return readJson<{ status: string; backend: CacheBackend }>(
    await fetch(apiUrl("/health")),
  );
}

export async function listServices(): Promise<ServiceInstance[]> {
  const data = await request<{ services: ServiceInstance[] }>(
    "/scopes/store/instances",
  );
  return data.services;
}

export async function listAgents(): Promise<AgentItem[]> {
  const data = await request<{ agents: AgentItem[] }>("/agents/list");
  return data.agents;
}

export async function listAgentServices(
  agentId: string,
): Promise<ServiceInstance[]> {
  const data = await request<{ services: ServiceInstance[] }>(
    `/scopes/agents/${encodeURIComponent(agentId)}/instances`,
  );
  return data.services;
}

export async function getServiceInstance(
  instanceId: string,
): Promise<ServiceInstance> {
  return request(`/instances/${encodeURIComponent(instanceId)}`);
}

export async function getServiceState(
  instanceId: string,
): Promise<ServiceState> {
  return request(`/instances/${encodeURIComponent(instanceId)}/state`);
}

export async function getInstanceAuthStatus(
  instanceId: string,
): Promise<AuthStatusView> {
  const data = await request<{ auth: AuthStatusView }>(
    `/instances/${encodeURIComponent(instanceId)}/auth`,
  );
  return data.auth;
}

export async function startInstanceAuthorization(
  instanceId: string,
): Promise<AuthOperationResult> {
  return request(`/instances/${encodeURIComponent(instanceId)}/auth/start`, {
    method: "POST",
  });
}

export async function refreshInstanceAuthorization(
  instanceId: string,
): Promise<AuthOperationResult> {
  return request(`/instances/${encodeURIComponent(instanceId)}/auth/refresh`, {
    method: "POST",
  });
}

export async function logoutInstanceAuthorization(
  instanceId: string,
): Promise<AuthOperationResult> {
  return request(`/instances/${encodeURIComponent(instanceId)}/auth/logout`, {
    method: "POST",
  });
}

export async function upgradeInstanceAuthorizationScope(
  instanceId: string,
  requiredScope: string,
): Promise<AuthOperationResult> {
  return request(
    `/instances/${encodeURIComponent(instanceId)}/auth/scope-upgrade`,
    {
      method: "POST",
      body: JSON.stringify({ required_scope: requiredScope }),
    },
  );
}

export async function listInstanceTools(
  instanceId: string,
  filter: ToolVisibilityFilter = "available",
): Promise<ToolInfo[]> {
  const data = await request<{ tools: ToolInfo[] }>(
    `/instances/${encodeURIComponent(instanceId)}/tools?filter=${filter}`,
  );
  return data.tools;
}

export async function setInstanceToolPolicy(
  instanceId: string,
  availableTools: string[],
) {
  return request(`/instances/${encodeURIComponent(instanceId)}/tool-policy`, {
    method: "PUT",
    body: JSON.stringify({ available_tools: availableTools }),
  });
}

export async function clearInstanceToolPolicy(instanceId: string) {
  return request(`/instances/${encodeURIComponent(instanceId)}/tool-policy`, {
    method: "DELETE",
  });
}

export async function listInstanceResources(
  instanceId: string,
): Promise<ResourceInfo[]> {
  const data = await request<{ resources: ResourceInfo[] }>(
    `/instances/${encodeURIComponent(instanceId)}/resources`,
  );
  return data.resources;
}

export async function listInstanceResourceTemplates(
  instanceId: string,
): Promise<ResourceTemplateInfo[]> {
  const data = await request<{ resource_templates: ResourceTemplateInfo[] }>(
    `/instances/${encodeURIComponent(instanceId)}/resource_templates`,
  );
  return data.resource_templates;
}

export async function listInstancePrompts(
  instanceId: string,
): Promise<PromptInfo[]> {
  const data = await request<{ prompts: PromptInfo[] }>(
    `/instances/${encodeURIComponent(instanceId)}/prompts`,
  );
  return data.prompts;
}

export async function readInstanceResource(instanceId: string, uri: string) {
  return request(
    `/instances/${encodeURIComponent(instanceId)}/read_resource${buildQuery({ uri })}`,
  );
}

export async function checkInstance(instanceId: string) {
  return request(`/instances/${encodeURIComponent(instanceId)}/check`);
}

export async function connectInstance(instanceId: string) {
  return request(`/instances/${encodeURIComponent(instanceId)}/connect`, {
    method: "POST",
  });
}

export async function disconnectInstance(instanceId: string) {
  return request(`/instances/${encodeURIComponent(instanceId)}/disconnect`, {
    method: "POST",
  });
}

export async function restartInstance(instanceId: string) {
  return request(`/instances/${encodeURIComponent(instanceId)}/restart`, {
    method: "POST",
  });
}

export async function callInstanceTool(
  instanceId: string,
  toolName: string,
  args: Record<string, unknown>,
) {
  return request(`/instances/${encodeURIComponent(instanceId)}/call`, {
    method: "POST",
    body: JSON.stringify({ tool_name: toolName, args }),
  });
}
