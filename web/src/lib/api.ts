export type CacheBackend = "memory" | "redis" | "openkeyv_memory" | "openkeyv_redis"

export type ConnectionStatus = "connected" | "disconnected" | "connecting" | "error"

export type AuthStatus =
  | "not_required"
  | "unauthenticated"
  | "authorizing"
  | "authenticated"
  | "refreshing"
  | "scope_upgrade_required"
  | "error"

export type AuthFlow = "authorization_code" | "client_credentials"

export type AuthStatusView = {
  instance_id: string
  status: AuthStatus
  flow?: AuthFlow
  scopes?: string[]
  required_scope?: string
}

export type AuthorizationStart = {
  instance_id: string
  authorization_url: string
  scopes: string[]
}

export type AuthOperationResult = {
  auth: AuthStatusView
  authorization?: AuthorizationStart | null
}

export type ScopeRef = { type: "store" } | { type: "agent"; agent_id: string }

export type ConfigRevision = {
  base_revision: number
  scope_revision: number
}

export type ToolInfo = {
  name: string
  title?: string | null
  description: string
  input_schema: unknown
  output_schema?: unknown
  annotations?: unknown
  meta?: unknown
}

export type ResourceInfo = {
  uri: string
  name: string
  title?: string
  description?: string
  mimeType?: string
  size?: number
  annotations?: unknown
  _meta?: unknown
}

export type PromptInfo = {
  name: string
  title?: string
  description?: string
  arguments?: unknown
  _meta?: unknown
}

export type ResourceTemplateInfo = {
  uriTemplate: string
  name: string
  title?: string
  description?: string
  mimeType?: string
  annotations?: unknown
  _meta?: unknown
}

export type McpServerCapabilities = {
  tools: boolean
  toolsListChanged: boolean
  resources: boolean
  resourcesSubscribe: boolean
  resourcesListChanged: boolean
  prompts: boolean
  promptsListChanged: boolean
  completions: boolean
  logging: boolean
  tasks: boolean
  taskList: boolean
  taskCancel: boolean
  taskToolCalls: boolean
  extensions?: Record<string, unknown>
  experimental?: Record<string, unknown>
}

export type McpServerMetadata = {
  protocolVersion: string
  serverInfo: {
    name: string
    title?: string
    version: string
    description?: string
    websiteUrl?: string
  }
  instructions?: string
  capabilities: McpServerCapabilities
}

export type ServiceInstance = {
  instance_id: string
  service_name: string
  scope: ScopeRef
  transport: string
  url: string | null
  command: string | null
  status: ConnectionStatus
  tools: ToolInfo[]
  effective_config: Record<string, unknown>
  config_revision: ConfigRevision
  applied_config_revision: ConfigRevision | null
  added_time: number
  mcp?: McpServerMetadata | null
}

export type ToolStatusItem = {
  tool_name: string
  status: string
}

export type InstanceStatus = {
  instance_id: string
  service_name: string
  scope: ScopeRef
  health_status: string
  last_health_check: number
  connection_attempts: number
  max_connection_attempts: number
  current_error: string | null
  tools: ToolStatusItem[]
  window_error_rate: number | null
  latency_p95: number | null
  latency_p99: number | null
  sample_size: number | null
  next_retry_time: number | null
  hard_deadline: number | null
  lease_deadline: number | null
}

export type AgentItem = {
  agent_id: string
  instance_ids: string[]
}

export type CacheReport = Record<string, unknown>
export type ConfigReport = Record<string, unknown>

export type UiLanguage = "auto" | "en" | "zh" | string

export type LogSettingsPayload = {
  max_size_bytes?: number | null
  retention_days?: number | null
}

export type SettingsPayload = {
  language?: UiLanguage
  default_backup_dir?: string
  logging?: LogSettingsPayload
  [key: string]: unknown
}

export type SettingsPathsPayload = {
  backup_dir_base?: string | null
  backup_dir_input?: string | null
  backup_dir_resolved?: string | null
  log_dir?: string | null
  log_file_name?: string | null
  log_file_path?: string | null
}

export type ConfigFilePayload = {
  path?: string
  format?: string
  content?: string
}

export type MetaPayload = {
  version?: string
  settings?: SettingsPayload
  settings_paths?: SettingsPathsPayload
  config_file?: ConfigFilePayload
  [key: string]: unknown
}

export type UpdateSettingsPayload = {
  language?: UiLanguage
  default_backup_dir?: string
  logging?: LogSettingsPayload
  [key: string]: unknown
}

export type ApiEnvelope<T> = {
  success: boolean
  message: string
  data?: T
  errors?: Array<{ code: string; message: string; field?: string }>
}

type FlexibleEnvelope<T> = ApiEnvelope<T> | { ok: boolean; message?: string; data?: T; error?: string }

export class ApiError extends Error {
  status: number

  constructor(message: string, status: number) {
    super(message)
    this.name = "ApiError"
    this.status = status
  }
}

export type ServiceStartupPolicy = "manual" | "lazy" | "on-store-start"
export type ServiceRestartPolicy = "no" | "on-failure" | `on-failure:${number}` | "always" | "unless-stopped"

export type ServiceLifecycleConfig = {
  startup_policy?: ServiceStartupPolicy
  restart_policy?: ServiceRestartPolicy
}

export type ScopeDescriptor = {
  config?: Record<string, unknown>
  lifecycle?: ServiceLifecycleConfig
}

export type AddServiceInput = {
  name: string
  scope: ScopeRef
  transport: "stdio" | "streamable-http" | "sse"
  commandOrUrl: string
  description?: string
  workingDir?: string
  env?: Record<string, string>
  headers?: Record<string, string>
  lifecycle?: ServiceLifecycleConfig
}

export type UpdateServiceScopeInput = {
  serviceName: string
  scope: ScopeRef
  transport: "stdio" | "streamable-http" | "sse"
  commandOrUrl: string
  description?: string
  workingDir?: string
  env?: Record<string, string>
  headers?: Record<string, string>
  lifecycle?: ServiceLifecycleConfig
}

const API_BASE = import.meta.env.VITE_MCPSTORE_API_BASE || "/api"

function apiUrl(path: string) {
  return `${API_BASE}${path}`
}

export function buildQuery(params: Record<string, string | number | boolean | null | undefined>) {
  const search = new URLSearchParams()
  for (const [key, value] of Object.entries(params)) {
    if (value === undefined || value === null || value === "") continue
    search.set(key, String(value))
  }
  const query = search.toString()
  return query ? `?${query}` : ""
}

async function readJson<T>(response: Response): Promise<T> {
  const text = await response.text()
  const body = text ? JSON.parse(text) : null
  if (!response.ok) {
    const message = body?.message || body?.errors?.[0]?.message || response.statusText
    throw new ApiError(message, response.status)
  }
  return body as T
}

export async function api<T>(path: string, options: RequestInit = {}): Promise<T> {
  const headers = new Headers(options.headers)
  headers.set("Accept", "application/json")

  if (options.body !== undefined && !(options.body instanceof FormData) && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json")
  }

  const payload = await readJson<T | FlexibleEnvelope<T>>(
    await fetch(apiUrl(path), {
      ...options,
      headers,
    }),
  )

  if (payload && typeof payload === "object" && "success" in payload) {
    const envelope = payload as ApiEnvelope<T>
    if (!envelope.success) throw new ApiError(envelope.errors?.[0]?.message || envelope.message, 200)
    return envelope.data as T
  }

  if (payload && typeof payload === "object" && "ok" in payload) {
    const envelope = payload as { ok: boolean; message?: string; data?: T; error?: string }
    if (!envelope.ok) throw new ApiError(envelope.error || envelope.message || "Request failed", 200)
    return envelope.data as T
  }

  return payload as T
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(apiUrl(path), {
    headers: { "Content-Type": "application/json", ...init?.headers },
    ...init,
  })
  const payload = await readJson<ApiEnvelope<T>>(response)
  if (!payload.success) {
    throw new ApiError(payload.errors?.[0]?.message || payload.message, response.status)
  }
  return payload.data as T
}

export async function health() {
  return readJson<{ status: string; backend: CacheBackend }>(await fetch(apiUrl("/health")))
}

export async function listServices(): Promise<ServiceInstance[]> {
  const data = await request<{ services: ServiceInstance[] }>("/scopes/store/instances")
  return data.services
}

export async function listAgents(): Promise<AgentItem[]> {
  const data = await request<{ agents: AgentItem[] }>("/agents/list")
  return data.agents
}

export async function listAgentServices(agentId: string): Promise<ServiceInstance[]> {
  const data = await request<{ services: ServiceInstance[] }>(`/scopes/agents/${encodeURIComponent(agentId)}/instances`)
  return data.services
}

export async function getServiceInstance(instanceId: string): Promise<ServiceInstance> {
  return request(`/instances/${encodeURIComponent(instanceId)}`)
}

export async function getInstanceStatus(instanceId: string): Promise<InstanceStatus> {
  return request(`/instances/${encodeURIComponent(instanceId)}/status`)
}

export async function getInstanceAuthStatus(instanceId: string): Promise<AuthStatusView> {
  const data = await request<{ auth: AuthStatusView }>(`/instances/${encodeURIComponent(instanceId)}/auth`)
  return data.auth
}

export async function startInstanceAuthorization(instanceId: string): Promise<AuthOperationResult> {
  return request(`/instances/${encodeURIComponent(instanceId)}/auth/start`, { method: "POST" })
}

export async function refreshInstanceAuthorization(instanceId: string): Promise<AuthOperationResult> {
  return request(`/instances/${encodeURIComponent(instanceId)}/auth/refresh`, { method: "POST" })
}

export async function logoutInstanceAuthorization(instanceId: string): Promise<AuthOperationResult> {
  return request(`/instances/${encodeURIComponent(instanceId)}/auth/logout`, { method: "POST" })
}

export async function upgradeInstanceAuthorizationScope(
  instanceId: string,
  requiredScope: string,
): Promise<AuthOperationResult> {
  return request(`/instances/${encodeURIComponent(instanceId)}/auth/scope-upgrade`, {
    method: "POST",
    body: JSON.stringify({ required_scope: requiredScope }),
  })
}

export async function listInstanceTools(instanceId: string): Promise<ToolInfo[]> {
  const data = await request<{ tools: ToolInfo[] }>(`/instances/${encodeURIComponent(instanceId)}/tools`)
  return data.tools
}

export async function listInstanceResources(instanceId: string): Promise<ResourceInfo[]> {
  const data = await request<{ resources: ResourceInfo[] }>(`/instances/${encodeURIComponent(instanceId)}/resources`)
  return data.resources
}

export async function listInstanceResourceTemplates(instanceId: string): Promise<ResourceTemplateInfo[]> {
  const data = await request<{ resource_templates: ResourceTemplateInfo[] }>(
    `/instances/${encodeURIComponent(instanceId)}/resource_templates`,
  )
  return data.resource_templates
}

export async function listInstancePrompts(instanceId: string): Promise<PromptInfo[]> {
  const data = await request<{ prompts: PromptInfo[] }>(`/instances/${encodeURIComponent(instanceId)}/prompts`)
  return data.prompts
}

export async function readInstanceResource(instanceId: string, uri: string) {
  return request(`/instances/${encodeURIComponent(instanceId)}/read_resource${buildQuery({ uri })}`)
}

export async function checkInstance(instanceId: string) {
  return request(`/instances/${encodeURIComponent(instanceId)}/check`)
}

export async function connectInstance(instanceId: string) {
  return request(`/instances/${encodeURIComponent(instanceId)}/connect`, { method: "POST" })
}

export async function disconnectInstance(instanceId: string) {
  return request(`/instances/${encodeURIComponent(instanceId)}/disconnect`, { method: "POST" })
}

export async function restartInstance(instanceId: string) {
  return request(`/instances/${encodeURIComponent(instanceId)}/restart`, { method: "POST" })
}

export async function callInstanceTool(instanceId: string, toolName: string, args: Record<string, unknown>) {
  return request(`/instances/${encodeURIComponent(instanceId)}/call`, {
    method: "POST",
    body: JSON.stringify({ tool_name: toolName, args }),
  })
}

export async function showConfig(options: { format?: string; instanceId?: string } = {}): Promise<ConfigReport> {
  const format = options.format ?? "native"
  return request(`/config${buildQuery({ format: format === "native" ? undefined : format, instance_id: options.instanceId })}`)
}

export async function showAgentConfig(
  agentId: string,
  options: { format?: string; instanceId?: string } = {},
): Promise<ConfigReport> {
  const format = options.format ?? "native"
  return request(
    `/scopes/agents/${encodeURIComponent(agentId)}/config${buildQuery({ format: format === "native" ? undefined : format, instance_id: options.instanceId })}`,
  )
}

export async function getMeta(): Promise<MetaPayload> {
  return api<MetaPayload>("/v1/meta")
}

export async function updateSettings(payload: UpdateSettingsPayload): Promise<SettingsPayload> {
  return api<SettingsPayload>("/v1/settings", {
    method: "PUT",
    body: JSON.stringify(payload),
  })
}

export async function resetConfig() {
  return request("/config/reset", { method: "POST" })
}

export async function resetAgentConfig(agentId: string) {
  return request(`/scopes/agents/${encodeURIComponent(agentId)}/reset`, { method: "POST" })
}

export async function cacheHealth(): Promise<CacheReport> {
  return request("/cache/health")
}

export async function cacheInspect(): Promise<CacheReport> {
  return request("/cache/inspect")
}

export async function switchCache(backend: CacheBackend) {
  return request("/cache/switch", {
    method: "POST",
    body: JSON.stringify({ backend }),
  })
}

export async function addService(input: AddServiceInput) {
  return addServiceFromConfig({
    name: input.name,
    scope: input.scope,
    config: buildServiceConfig(input),
    lifecycle: input.lifecycle,
  })
}

export async function addServiceFromConfig(input: {
  name: string
  scope: ScopeRef
  config: Record<string, unknown>
  lifecycle?: ServiceLifecycleConfig
}) {
  const descriptor: ScopeDescriptor = {}
  const scopes =
    input.scope.type === "store"
      ? { store: descriptor }
      : { agents: { [input.scope.agent_id]: descriptor } }
  const existing =
    input.config._mcpstore && typeof input.config._mcpstore === "object"
      ? (input.config._mcpstore as Record<string, unknown>)
      : {}
  const payload = {
    ...input.config,
    _mcpstore: {
      ...existing,
      scopes,
      ...(input.lifecycle ? { lifecycle: input.lifecycle } : {}),
    },
  }
  return request(`/services/${encodeURIComponent(input.name)}`, {
    method: "POST",
    body: JSON.stringify(payload),
  })
}

export async function declareAgentServiceScope(agentId: string, serviceName: string, descriptor: ScopeDescriptor = {}) {
  return request(`/services/${encodeURIComponent(serviceName)}/scopes/agents/${encodeURIComponent(agentId)}`, {
    method: "PUT",
    body: JSON.stringify(descriptor),
  })
}

export async function removeServiceScope(serviceName: string, scope: ScopeRef) {
  const path =
    scope.type === "store"
      ? `/services/${encodeURIComponent(serviceName)}/scopes/store`
      : `/services/${encodeURIComponent(serviceName)}/scopes/agents/${encodeURIComponent(scope.agent_id)}`
  return request(path, { method: "DELETE" })
}

export async function updateServiceScope(input: UpdateServiceScopeInput) {
  const descriptor: ScopeDescriptor = {
    config: buildServiceConfig(input),
    ...(input.lifecycle ? { lifecycle: input.lifecycle } : {}),
  }
  const path =
    input.scope.type === "store"
      ? `/services/${encodeURIComponent(input.serviceName)}/scopes/store`
      : `/services/${encodeURIComponent(input.serviceName)}/scopes/agents/${encodeURIComponent(input.scope.agent_id)}`
  return request(path, {
    method: "PUT",
    body: JSON.stringify(descriptor),
  })
}

export function parseKvLines(value: string) {
  return value
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .reduce<Record<string, string>>((acc, line) => {
      const index = line.indexOf("=")
      if (index <= 0) {
        throw new Error(`Invalid key/value line: ${line}`)
      }
      acc[line.slice(0, index).trim()] = line.slice(index + 1).trim()
      return acc
    }, {})
}

export function formatKvLines(value?: Record<string, unknown>) {
  if (!value) return ""
  return Object.entries(value)
    .map(([key, item]) => `${key}=${String(item ?? "")}`)
    .join("\n")
}

function buildServiceConfig(
  input: Pick<
    UpdateServiceScopeInput,
    "transport" | "commandOrUrl" | "description" | "workingDir" | "env" | "headers"
  >,
) {
  const common = {
    transport: input.transport,
    env: input.env || {},
    headers: input.headers || {},
    workingDir: input.workingDir || undefined,
    description: input.description || undefined,
  }
  if (input.transport === "stdio") {
    const command = input.commandOrUrl.split(/\s+/).filter(Boolean)
    return {
      ...common,
      command: command[0],
      args: command.slice(1),
    }
  }

  return {
    ...common,
    url: input.commandOrUrl,
  }
}
