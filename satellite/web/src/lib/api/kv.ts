// KV state belongs to the selected daemon resource.
import { api, buildQuery } from "./client"
import type {
  AddServiceInput,
  AgentItem,
  AuthFlow,
  AuthOperationResult,
  AuthStatus,
  AuthStatusView,
  AuthorizationStart,
  KvStoreName,
  KvReport,
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
} from "../api"

// KV state belongs to the selected daemon resource.
export async function kvHealth(): Promise<KvReport> {
  return api("/kv/health")
}

export async function kvInspect(): Promise<KvReport> {
  return api("/kv/inspect")
}

export async function switchKv(store: KvStoreName) {
  // Rust KvSwitchRequest = { store, config }; the frontend has no config editing UI, so config is an empty object that takes store defaults.
  return api("/kv/switch", {
    method: "POST",
    body: JSON.stringify({ store, config: {} }),
  })
}
