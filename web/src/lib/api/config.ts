// 本模块全部为 app 自有接口（聚合服务 spawn 本地子进程 + 读取并导入本地编辑器配置），
// 固定走 appRequest（本 app 进程），不随 core 后端切换 —— 见 接口文档 §附录C。
import { appRequest as request, buildQuery } from "./client";
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

export type McpHubOptions = {
  scope?: "store" | "agent";
  agentId?: string;
  transport: "stdio" | "streamable-http";
  host?: string;
  port?: number;
  path?: string;
  instanceId?: string;
  sessionKey?: string;
};

export type McpHubStatus = {
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

function mcpHubQuery(options: McpHubOptions) {
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

export async function getMcpHubDescriptor(options: McpHubOptions) {
  return request<{
    transport: string;
    command: string | null;
    args: string[];
    url: string | null;
  }>(`/mcp-hub/descriptor${mcpHubQuery(options)}`);
}

export async function getMcpHubStatus(options: McpHubOptions) {
  return request<McpHubStatus>(`/mcp-hub/status${mcpHubQuery(options)}`);
}

export async function startMcpHub(options: McpHubOptions) {
  return request<{ running: boolean; pid: number | null; transport: string; url: string | null }>(
    `/mcp-hub/start${mcpHubQuery(options)}`,
    { method: "POST" },
  );
}

export async function stopMcpHub() {
  return request<{ running: boolean; pid?: number | null }>("/mcp-hub/stop", {
    method: "POST",
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
