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

export type AggregateOptions = {
  scope?: "store" | "agent";
  agentId?: string;
  transport: "stdio" | "streamable-http";
  host?: string;
  port?: number;
  path?: string;
  instanceId?: string;
  sessionKey?: string;
};

export type AggregateStatus = {
  running: boolean;
  pid: number | null;
  background_supported: boolean;
  transport: string;
  host: string;
  port: number;
  path: string;
  url: string | null;
  command: string | null;
  args: string[];
};

function aggregateQuery(options: AggregateOptions) {
  return buildQuery({
    scope: options.scope,
    agent_id: options.agentId,
    transport: options.transport,
    host: options.host,
    port: options.port,
    path: options.path,
    instance_id: options.instanceId,
    session_key: options.sessionKey,
  });
}

export async function getAggregateLaunch(options: AggregateOptions) {
  return request<{
    transport: string;
    command: string | null;
    args: string[];
    url: string | null;
  }>(`/aggregate/launch${aggregateQuery(options)}`);
}

export async function getAggregateStatus(options: AggregateOptions) {
  return request<AggregateStatus>(`/aggregate/status${aggregateQuery(options)}`);
}

export async function startAggregate(options: AggregateOptions) {
  return request<{ running: boolean; pid: number | null; transport: string; url: string | null }>(
    `/aggregate/start${aggregateQuery(options)}`,
    { method: "POST" },
  );
}

export async function stopAggregate() {
  return request<{ running: boolean; pid?: number | null }>("/aggregate/stop", {
    method: "POST",
  });
}

type ClientConfigInspectPayload = {
  client: string;
  path: string;
  format: string;
  content_hash: string;
  services: Array<{ name: string; fields: string[] }>;
  unsupported_fields: string[];
};

type ClientConfigPlanPayload = {
  client: string;
  path: string;
  content_hash: string;
  plans: Array<{
    name: string;
    kind: string;
    status: string;
    fields: string[];
    unsupported_fields: string[];
  }>;
};

export async function inspectClientConfig(
  client: string,
  path: string,
): Promise<ClientConfigInspectPayload> {
  return request("/client-config/inspect", {
    method: "POST",
    body: JSON.stringify({ client, path }),
  });
}

export async function planClientConfig(
  client: string,
  path: string,
  entries: unknown[],
): Promise<ClientConfigPlanPayload> {
  return request("/client-config/plan", {
    method: "POST",
    body: JSON.stringify({ client, path, entries }),
  });
}

export async function applyClientConfig(
  client: string,
  path: string,
  expectedHash: string,
  entries: unknown[],
) {
  return request<{
    changed: boolean;
    change_id?: string;
    plans: ClientConfigPlanPayload["plans"];
  }>("/client-config/apply", {
    method: "POST",
    body: JSON.stringify({
      client,
      path,
      expected_hash: expectedHash,
      entries,
    }),
  });
}

export async function undoClientConfig(changeId: string) {
  return request<{ changed: boolean }>("/client-config/undo", {
    method: "POST",
    body: JSON.stringify({ change_id: changeId }),
  });
}

export async function importClientServices(
  client: string,
  path: string,
  names: string[],
) {
  return request<{ imported: Array<{ name: string; transport: string }> }>(
    "/client-config/import",
    {
      method: "POST",
      body: JSON.stringify({ client, path, names }),
    },
  );
}
