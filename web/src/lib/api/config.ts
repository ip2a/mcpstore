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

export async function getAggregateLaunch(options: {
  transport: "stdio" | "streamable-http";
  instanceId?: string;
  sessionKey?: string;
}) {
  const query = buildQuery({
    transport: options.transport,
    instance_id: options.instanceId,
    session_key: options.sessionKey,
  });
  return request<{
    transport: string;
    command: string | null;
    args: string[];
    url: string | null;
  }>(`/aggregate/launch${query}`);
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
