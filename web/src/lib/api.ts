export type CacheBackend = "memory" | "redis" | "openkeyv_memory" | "openkeyv_redis"

export type ConnectionStatus = "Connected" | "Disconnected" | "Connecting" | "Error" | string

export type ToolInfo = {
  name: string
  description?: string
  schema?: unknown
  service_name?: string
  service?: string
  agent_id?: string
  [key: string]: unknown
}

export type ServiceEntry = {
  name: string
  original_name?: string
  transport?: string
  status?: ConnectionStatus
  tools?: ToolInfo[]
  config?: Record<string, unknown>
  agent_id?: string
  added_time?: number
  url?: string
  command?: string
  [key: string]: unknown
}

export type AgentItem = {
  agent_id?: string
  id?: string
  services?: string[]
  service_names?: string[]
  [key: string]: unknown
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

export type AddServiceInput = {
  name: string
  scope: "store" | "agent"
  agentId?: string
  transport: "stdio" | "streamable-http" | "sse"
  commandOrUrl: string
  description?: string
  workingDir?: string
  env?: Record<string, string>
  headers?: Record<string, string>
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

export async function listServices(): Promise<ServiceEntry[]> {
  const data = await request<{ services?: ServiceEntry[] }>("/for_store/list_services")
  return data?.services || []
}

export async function listAgents(): Promise<AgentItem[]> {
  const data = await request<{ agents?: AgentItem[] }>("/agents/list")
  return data?.agents || []
}

export async function listTools(serviceName?: string): Promise<ToolInfo[]> {
  const suffix = buildQuery({ service_name: serviceName })
  const data = await request<{ tools?: ToolInfo[] }>(`/for_store/list_tools${suffix}`)
  return data?.tools || []
}

export async function listAgentServices(agentId: string): Promise<ServiceEntry[]> {
  const data = await request<{ services?: ServiceEntry[] }>(`/for_agent/${encodeURIComponent(agentId)}/list_services`)
  return data?.services || []
}

export async function listAgentTools(agentId: string, serviceName?: string): Promise<ToolInfo[]> {
  const suffix = buildQuery({ service_name: serviceName })
  const data = await request<{ tools?: ToolInfo[] }>(`/for_agent/${encodeURIComponent(agentId)}/list_tools${suffix}`)
  return data?.tools || []
}

export async function assignService(agentId: string, serviceName: string) {
  return request(`/for_agent/${encodeURIComponent(agentId)}/assign_service/${encodeURIComponent(serviceName)}`, { method: "POST" })
}

export async function unassignService(agentId: string, serviceName: string) {
  return request(`/for_agent/${encodeURIComponent(agentId)}/unassign_service/${encodeURIComponent(serviceName)}`, { method: "POST" })
}

export async function serviceInfo(name: string): Promise<ServiceEntry> {
  return request(`/for_store/service_info/${encodeURIComponent(name)}`)
}

export async function serviceStatus(name: string): Promise<unknown> {
  return request(`/for_store/service_status/${encodeURIComponent(name)}`)
}

export async function checkServices(): Promise<unknown> {
  return request("/for_store/check_services")
}

export async function showConfig(options: { format?: string } = {}): Promise<ConfigReport> {
  const format = options.format ?? "native"
  const query = format === "native" ? "" : `?format=${encodeURIComponent(format)}`
  return request(`/for_store/show_config${query}`)
}

export async function showAgentConfig(agentId: string, options: { format?: string } = {}): Promise<ConfigReport> {
  const format = options.format ?? "native"
  const query = format === "native" ? "" : `?format=${encodeURIComponent(format)}`
  return request(`/for_agent/${encodeURIComponent(agentId)}/show_config${query}`)
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
  return request("/for_store/reset_config", { method: "POST" })
}

export async function resetAgentConfig(agentId: string) {
  return request(`/for_agent/${encodeURIComponent(agentId)}/reset_config`, { method: "POST" })
}

export async function cacheHealth(): Promise<CacheReport> {
  return request("/cache/health")
}

export async function cacheInspect(): Promise<CacheReport> {
  return request("/cache/inspect")
}

export async function connectService(name: string) {
  return request(`/for_store/connect_service/${encodeURIComponent(name)}`, { method: "POST" })
}

export async function disconnectService(name: string) {
  return request(`/for_store/disconnect_service/${encodeURIComponent(name)}`, { method: "POST" })
}

export async function restartService(name: string) {
  return request(`/for_store/restart_service/${encodeURIComponent(name)}`, { method: "POST" })
}

export async function removeService(name: string) {
  return request(`/for_store/remove_service/${encodeURIComponent(name)}`, { method: "POST" })
}

export async function switchCache(backend: CacheBackend) {
  return request("/cache/switch", {
    method: "POST",
    body: JSON.stringify({ backend }),
  })
}

export async function callTool(serviceName: string, toolName: string, args: Record<string, unknown>) {
  return request("/for_store/call_tool", {
    method: "POST",
    body: JSON.stringify({ tool_name: `${serviceName}_${toolName}`, args }),
  })
}

export async function callStoreTool(toolName: string, args: Record<string, unknown>) {
  return request("/for_store/call_tool", {
    method: "POST",
    body: JSON.stringify({ tool_name: toolName, args }),
  })
}

export async function callAgentTool(agentId: string, toolName: string, args: Record<string, unknown>) {
  return request(`/for_agent/${encodeURIComponent(agentId)}/call_tool`, {
    method: "POST",
    body: JSON.stringify({ tool_name: toolName, args }),
  })
}

export async function addService(input: AddServiceInput) {
  const isStdio = input.transport === "stdio"
  const payload = isStdio
    ? {
        name: input.name,
        command: input.commandOrUrl.split(/\s+/).filter(Boolean)[0],
        args: input.commandOrUrl.split(/\s+/).filter(Boolean).slice(1),
        transport: "stdio",
        env: input.env || {},
        headers: input.headers || {},
        working_dir: input.workingDir || undefined,
        description: input.description || undefined,
      }
    : {
        name: input.name,
        url: input.commandOrUrl,
        transport: input.transport,
        env: input.env || {},
        headers: input.headers || {},
        working_dir: input.workingDir || undefined,
        description: input.description || undefined,
      }

  if (input.scope === "agent" && input.agentId) {
    return request(`/for_agent/${encodeURIComponent(input.agentId)}/add_service`, {
      method: "POST",
      body: JSON.stringify(payload),
    })
  }

  return request("/for_store/add_service", {
    method: "POST",
    body: JSON.stringify(payload),
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
