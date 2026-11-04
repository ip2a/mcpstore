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
  // Backend CacheSwitchRequest = { store, config }; the frontend has no config editing UI, so config is an empty object that takes backend defaults.
  return api("/cache/switch", {
    method: "POST",
    body: JSON.stringify({ store: backend, config: {} }),
  });
}
