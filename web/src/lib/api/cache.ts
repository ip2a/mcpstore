import { api, appRequest, buildQuery } from "./client";
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

// /cache/* 是 app 面接口（core 面无此路由），固定走本地 /api，不随 core base 切换。
export async function cacheHealth(): Promise<CacheReport> {
  return appRequest("/cache/health");
}

export async function cacheInspect(): Promise<CacheReport> {
  return appRequest("/cache/inspect");
}

export async function switchCache(backend: CacheBackend) {
  // 后端 CacheSwitchRequest = { store, config }；前端无配置编辑 UI，config 传空对象走后端默认。
  return appRequest("/cache/switch", {
    method: "POST",
    body: JSON.stringify({ store: backend, config: {} }),
  });
}
