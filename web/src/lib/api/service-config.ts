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

export async function addService(input: AddServiceInput) {
  return addServiceFromConfig({
    name: input.name,
    scope: input.scope,
    config: buildServiceConfig(input),
    lifecycle: input.lifecycle,
  });
}

export async function addServiceFromConfig(input: {
  name: string;
  scope: ScopeRef;
  config: Record<string, unknown>;
  lifecycle?: ServiceLifecycleConfig;
}) {
  const descriptor: ScopeDescriptor = {};
  const scopes =
    input.scope.type === "store"
      ? { store: descriptor }
      : { agents: { [input.scope.agent_id]: descriptor } };
  const existing =
    input.config._mcpstore && typeof input.config._mcpstore === "object"
      ? (input.config._mcpstore as Record<string, unknown>)
      : {};
  const payload = {
    ...input.config,
    _mcpstore: {
      ...existing,
      scopes,
      ...(input.lifecycle ? { lifecycle: input.lifecycle } : {}),
    },
  };
  return request(`/services/${encodeURIComponent(input.name)}`, {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export async function declareAgentServiceScope(
  agentId: string,
  serviceName: string,
  descriptor: ScopeDescriptor = {},
) {
  return request(
    `/services/${encodeURIComponent(serviceName)}/scopes/agents/${encodeURIComponent(agentId)}`,
    {
      method: "PUT",
      body: JSON.stringify(descriptor),
    },
  );
}

export async function removeServiceScope(serviceName: string, scope: ScopeRef) {
  const path =
    scope.type === "store"
      ? `/services/${encodeURIComponent(serviceName)}/scopes/store`
      : `/services/${encodeURIComponent(serviceName)}/scopes/agents/${encodeURIComponent(scope.agent_id)}`;
  return request(path, { method: "DELETE" });
}

export async function updateServiceScope(input: UpdateServiceScopeInput) {
  const descriptor: ScopeDescriptor = {
    config: buildServiceConfig(input),
    ...(input.lifecycle ? { lifecycle: input.lifecycle } : {}),
  };
  const path =
    input.scope.type === "store"
      ? `/services/${encodeURIComponent(input.serviceName)}/scopes/store`
      : `/services/${encodeURIComponent(input.serviceName)}/scopes/agents/${encodeURIComponent(input.scope.agent_id)}`;
  return request(path, {
    method: "PUT",
    body: JSON.stringify(descriptor),
  });
}

export function parseKvLines(value: string) {
  return value
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .reduce<Record<string, string>>((acc, line) => {
      const index = line.indexOf("=");
      if (index <= 0) {
        throw new Error(`Invalid key/value line: ${line}`);
      }
      acc[line.slice(0, index).trim()] = line.slice(index + 1).trim();
      return acc;
    }, {});
}

export function formatKvLines(value?: Record<string, unknown>) {
  if (!value) return "";
  return Object.entries(value)
    .map(([key, item]) => `${key}=${String(item ?? "")}`)
    .join("\n");
}

function buildServiceConfig(
  input: Pick<
    UpdateServiceScopeInput,
    | "transport"
    | "commandOrUrl"
    | "description"
    | "workingDir"
    | "env"
    | "headers"
  >,
) {
  const common = {
    transport: input.transport,
    env: input.env || {},
    headers: input.headers || {},
    workingDir: input.workingDir || undefined,
    description: input.description || undefined,
  };
  if (input.transport === "stdio") {
    const command = input.commandOrUrl.split(/\s+/).filter(Boolean);
    return {
      ...common,
      command: command[0],
      args: command.slice(1),
    };
  }

  return {
    ...common,
    url: input.commandOrUrl,
  };
}
