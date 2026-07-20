export type CacheBackend =
  "memory" | "redis" | "openkeyv_memory" | "openkeyv_redis";

export type DesiredState = "running" | "stopped";
export type RuntimePhase = "stopped" | "starting" | "running" | "stopping";
export type HealthState = "unknown" | "healthy" | "degraded" | "unhealthy";
export type RecoveryState =
  | { status: "idle" }
  | {
      status: "waiting";
      attempt: number;
      retry_at: number;
      hard_deadline: number;
    }
  | { status: "probing"; attempt: number; hard_deadline: number }
  | { status: "exhausted"; attempts: number };
export type ServiceAuthState =
  | { status: "not_required" }
  | { status: "unauthenticated" }
  | { status: "authorizing" }
  | { status: "authenticated" }
  | { status: "refreshing" }
  | { status: "scope_upgrade_required"; required_scope: string | null }
  | { status: "failed" };
export type ToolsStatus = "unknown" | "syncing" | "ready" | "stale" | "failed";
export type ToolAvailability = "available" | "unavailable";
export type ReadinessStatus = "unknown" | "ready" | "not_ready";
export type ReadinessReason =
  | "unknown"
  | "stopped"
  | "starting"
  | "recovering"
  | "unhealthy"
  | "auth_required"
  | "tools_not_ready"
  | "ready";
export type FailurePhase =
  "start" | "transport" | "health" | "recovery" | "auth" | "tools";
export type FailureInfo = {
  phase: FailurePhase;
  code: string;
  retryable: boolean;
  message: string;
  since: number;
};
export type ServiceState = {
  instance_id: string;
  service_name: string;
  scope: ScopeRef;
  desired: DesiredState;
  phase: RuntimePhase;
  health: HealthState;
  health_metrics: {
    error_rate: number | null;
    latency_p95_ms: number | null;
    latency_p99_ms: number | null;
    sample_size: number;
  };
  last_observed_at: number | null;
  recovery: RecoveryState;
  auth: ServiceAuthState;
  tools: {
    status: ToolsStatus;
    items: Array<{ name: string; availability: ToolAvailability }>;
  };
  readiness: {
    status: ReadinessStatus;
    reason: ReadinessReason;
    message: string | null;
    last_transition_at: number;
  };
  failure: FailureInfo | null;
  version: number;
  updated_at: number;
};

export type AuthStatus =
  | "not_required"
  | "unauthenticated"
  | "authorizing"
  | "authenticated"
  | "refreshing"
  | "scope_upgrade_required"
  | "error";

export type AuthFlow = "authorization_code" | "client_credentials";

export type AuthStatusView = {
  instance_id: string;
  status: AuthStatus;
  flow?: AuthFlow;
  scopes?: string[];
  required_scope?: string;
};

export type AuthorizationStart = {
  instance_id: string;
  authorization_url: string;
  scopes: string[];
};

export type AuthOperationResult = {
  auth: AuthStatusView;
  authorization?: AuthorizationStart | null;
};

export type ScopeRef = { type: "store" } | { type: "agent"; agent_id: string };

export type ConfigRevision = {
  base_revision: number;
  scope_revision: number;
};

export type ToolVisibilityFilter = "all" | "available" | "removed";

export type ToolInfo = {
  name: string;
  title?: string | null;
  description: string;
  input_schema: unknown;
  output_schema?: unknown;
  annotations?: unknown;
  meta?: unknown;
};

export type ResourceInfo = {
  uri: string;
  name: string;
  title?: string;
  description?: string;
  mimeType?: string;
  size?: number;
  annotations?: unknown;
  _meta?: unknown;
};

export type PromptInfo = {
  name: string;
  title?: string;
  description?: string;
  arguments?: unknown;
  _meta?: unknown;
};

export type ResourceTemplateInfo = {
  uriTemplate: string;
  name: string;
  title?: string;
  description?: string;
  mimeType?: string;
  annotations?: unknown;
  _meta?: unknown;
};

export type McpServerCapabilities = {
  tools: boolean;
  toolsListChanged: boolean;
  resources: boolean;
  resourcesSubscribe: boolean;
  resourcesListChanged: boolean;
  prompts: boolean;
  promptsListChanged: boolean;
  completions: boolean;
  logging: boolean;
  tasks: boolean;
  taskList: boolean;
  taskCancel: boolean;
  taskToolCalls: boolean;
  extensions?: Record<string, unknown>;
  experimental?: Record<string, unknown>;
};

export type McpServerMetadata = {
  protocolVersion: string;
  serverInfo: {
    name: string;
    title?: string;
    version: string;
    description?: string;
    websiteUrl?: string;
  };
  instructions?: string;
  capabilities: McpServerCapabilities;
};

export type ServiceInstance = {
  instance_id: string;
  service_name: string;
  scope: ScopeRef;
  transport: string;
  url: string | null;
  command: string | null;
  state: ServiceState;
  tools: ToolInfo[];
  effective_config: Record<string, unknown>;
  config_revision: ConfigRevision;
  applied_config_revision: ConfigRevision | null;
  added_time: number;
  mcp?: McpServerMetadata | null;
};

export type AgentItem = {
  agent_id: string;
  instance_ids: string[];
};

export type CacheReport = Record<string, unknown>;
export type ConfigReport = Record<string, unknown>;

export type UiLanguage = "auto" | "en" | "zh" | string;

export type LogSettingsPayload = {
  max_size_bytes?: number | null;
  retention_days?: number | null;
};

export type SettingsPayload = {
  language?: UiLanguage;
  default_backup_dir?: string;
  logging?: LogSettingsPayload;
  diagnostics?: DiagnosticsSettingsPayload;
  [key: string]: unknown;
};

export type DiagnosticsSettingsPayload = {
  enabled?: boolean;
  runtime_log?: {
    enabled?: boolean;
    max_size_bytes?: number;
  };
  history?: {
    enabled?: boolean;
    storage?: "memory" | "disk";
    max_records?: number;
    max_size_bytes?: number;
    retention_days?: number | null;
    payload?: "none" | "metadata" | "full";
  };
};

export type SettingsPathsPayload = {
  backup_dir_base?: string | null;
  backup_dir_input?: string | null;
  backup_dir_resolved?: string | null;
  log_dir?: string | null;
  log_file_name?: string | null;
  log_file_path?: string | null;
};

export type ConfigFilePayload = {
  path?: string;
  format?: string;
  content?: string;
};

export type MetaPayload = {
  version?: string;
  settings?: SettingsPayload;
  settings_paths?: SettingsPathsPayload;
  config_file?: ConfigFilePayload;
  [key: string]: unknown;
};

export type UpdateSettingsPayload = {
  language?: UiLanguage;
  default_backup_dir?: string;
  logging?: LogSettingsPayload;
  diagnostics?: DiagnosticsSettingsPayload;
  [key: string]: unknown;
};

export type ApiEnvelope<T> = {
  success: boolean;
  message: string;
  data?: T;
  errors?: Array<{ code: string; message: string; field?: string }>;
};

type FlexibleEnvelope<T> =
  ApiEnvelope<T> | { ok: boolean; message?: string; data?: T; error?: string };

export class ApiError extends Error {
  status: number;

  constructor(message: string, status: number) {
    super(message);
    this.name = "ApiError";
    this.status = status;
  }
}

export type ServiceStartupPolicy = "manual" | "lazy" | "on-store-start";
export type ServiceRestartPolicy =
  "no" | "on-failure" | `on-failure:${number}` | "always" | "unless-stopped";

export type ServiceLifecycleConfig = {
  startup_policy?: ServiceStartupPolicy;
  restart_policy?: ServiceRestartPolicy;
};

export type ScopeDescriptor = {
  config?: Record<string, unknown>;
  lifecycle?: ServiceLifecycleConfig;
};

export type AddServiceInput = {
  name: string;
  scope: ScopeRef;
  transport: "stdio" | "streamable-http";
  commandOrUrl: string;
  description?: string;
  workingDir?: string;
  env?: Record<string, string>;
  headers?: Record<string, string>;
  lifecycle?: ServiceLifecycleConfig;
};

export type UpdateServiceScopeInput = {
  serviceName: string;
  scope: ScopeRef;
  transport: "stdio" | "streamable-http";
  commandOrUrl: string;
  description?: string;
  workingDir?: string;
  env?: Record<string, string>;
  headers?: Record<string, string>;
  lifecycle?: ServiceLifecycleConfig;
};

export type ClientConfigInspectPayload = {
  client: string;
  path: string;
  format: string;
  content_hash: string;
  services: Array<{ name: string; fields: string[] }>;
  unsupported_fields: string[];
};

export type ClientConfigPlanPayload = {
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

export {
  health,
  listServices,
  listAgents,
  listAgentServices,
  getServiceInstance,
  getServiceState,
  getInstanceAuthStatus,
  startInstanceAuthorization,
  refreshInstanceAuthorization,
  logoutInstanceAuthorization,
  upgradeInstanceAuthorizationScope,
  listInstanceTools,
  setInstanceToolPolicy,
  clearInstanceToolPolicy,
  listInstanceResources,
  listInstanceResourceTemplates,
  listInstancePrompts,
  readInstanceResource,
  checkInstance,
  connectInstance,
  disconnectInstance,
  restartInstance,
  callInstanceTool,
} from "./api/service";
export {
  getAggregateLaunch,
  inspectClientConfig,
  planClientConfig,
  applyClientConfig,
  undoClientConfig,
  importClientServices,
} from "./api/config";
export {
  showConfig,
  showAgentConfig,
  getMeta,
  updateSettings,
  resetConfig,
  resetAgentConfig,
} from "./api/settings";
export { cacheHealth, cacheInspect, switchCache } from "./api/cache";
export {
  addService,
  addServiceFromConfig,
  declareAgentServiceScope,
  removeServiceScope,
  updateServiceScope,
  parseKvLines,
  formatKvLines,
} from "./api/service-config";
