// Cache state belongs to the selected daemon resource.
import { api, buildQuery } from "./client";
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

// Cache state belongs to the selected daemon resource.
export async function cacheHealth(): Promise<CacheReport> {
  return api("/cache/health");
}

export async function cacheInspect(): Promise<CacheReport> {
  return api("/cache/inspect");
}

export async function switchCache(backend: CacheBackend) {
  // 后端 CacheSwitchRequest = { store, config }；前端无配置编辑 UI，config 传空对象走后端默认。
  return api("/cache/switch", {
    method: "POST",
    body: JSON.stringify({ store: backend, config: {} }),
  });
}
