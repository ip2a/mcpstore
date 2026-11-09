// Panel-local client discovery and wiring stays on the App face.
import { api, appApi, buildQuery } from "./client"
export type DetectedBy = "config_file" | "config_dir" | "command"

export type ClientInventory = {
  id: string
  label: string
  version?: string | null
  detected_by: DetectedBy | null
  config_path: string
  /** Resolved CLI binary on PATH when available; null if not found. */
  executable_path?: string | null
  config_exists: boolean
  mcp_servers: string[] | null
  unsupported_fields: string[]
  wired: boolean
  wired_url: string | null
}

export type GatewayStatus = {
  running: boolean
  pid: number | null
  host: string
  port: number
  path: string
  url: string
  summary: unknown
}

export type GatewayRequestRecord = {
  seq: number
  at_unix_ms: number
  caller: string | null
  kind: "tool" | "prompt" | "resource"
  name: string
  instance_id: string
  service_name: string
  duration_ms: number
  success: boolean
}

export type GatewayRequestsPage = {
  running: boolean
  requests: GatewayRequestRecord[]
  latest_seq: number
}

export type ScopeRef = { type: "store" } | { type: "agent"; agent_id: string }

export type ExposeConfig = {
  session_state: boolean
  tool: boolean
  prompt: boolean
  resource: boolean
  openapi: boolean
  service: boolean
  cache: boolean
  search: boolean
}

export type WiredEntry = {
  client_id: string
  path: string
  name: string
  url: string
  applied_at_unix_ms: number
  // Effective values injected by the backend (default config derives to Agent{agent_id:client_id} + all false).
  scope: ScopeRef
  expose: ExposeConfig
}

/** Scope/expose config that can be overridden on wire; omit for backend defaults (dedicated scope + all false). */
export type WireOptions = {
  scope?: "store" | "agent"
  agentId?: string
  expose?: Partial<ExposeConfig>
}

/**
 * Wire result: either wired normally (entry written or adopted as-is), or an
 * existing mcpstore entry with different content was found — includes the path
 * and current/expected values; the frontend confirms and re-sends with force to replace.
 */
export type WireResult =
  | ({ outcome: "wired" } & WiredEntry)
  | { outcome: "conflict"; path: string; current: unknown; expected: unknown }

export function listClients(): Promise<ClientInventory[]> {
  return appApi<{ clients: ClientInventory[] }>("/clients").then((payload) => payload.clients)
}

export function getClientVersion(clientId: string): Promise<string | null> {
  return appApi<{ version: string | null }>(`/clients/${clientId}/version`).then(
    (payload) => payload.version,
  )
}

/** When path/url are omitted, the backend discovers the config file and generates the caller URL. */
export function wireClient(
  client: string,
  options?: WireOptions,
  force = false,
): Promise<WireResult> {
  const expose = options?.expose
  const query = buildQuery({
    client,
    scope: options?.scope,
    agent_id: options?.agentId,
    expose_session_state: expose?.session_state,
    expose_tool: expose?.tool,
    expose_prompt: expose?.prompt,
    expose_resource: expose?.resource,
    expose_openapi: expose?.openapi,
    expose_service: expose?.service,
    expose_cache: expose?.cache,
    expose_search: expose?.search,
    force: force || undefined,
  })
  return appApi(`/gateway/wire${query}`, { method: "POST" })
}

export function unwireClient(client: string): Promise<WiredEntry> {
  return appApi(`/gateway/unwire?client=${client}`, { method: "POST" })
}

export function getGatewayStatus(): Promise<GatewayStatus> {
  return api("/gateway/status")
}

/** wait_ms enables long polling (backend cap 25s). */
export function getGatewayRequests(after: number, waitMs?: number): Promise<GatewayRequestsPage> {
  return api(`/gateway/requests${buildQuery({ after, wait_ms: waitMs })}`)
}

export function getWiredClients(): Promise<WiredEntry[]> {
  return appApi<WiredEntry[]>("/gateway/wired")
}
