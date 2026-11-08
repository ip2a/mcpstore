import { ApiError, type ApiEnvelope, type ScopeRef } from "../api"
import { getAppApiBase, getApiBase, getSelectedHost } from "./backend"

const PANEL_TOKEN_STORAGE_KEY = "mcpstore.panel.token"

export function getPanelToken(): string {
  return localStorage.getItem(PANEL_TOKEN_STORAGE_KEY) ?? ""
}

export function setPanelToken(token: string) {
  localStorage.setItem(PANEL_TOKEN_STORAGE_KEY, token.trim())
}

/** Fired when the panel API answers 401 panel_auth_required; PanelTokenGate listens. */
export const PANEL_AUTH_REQUIRED_EVENT = "mcpstore:panel-auth-required"


function apiUrl(path: string) {
  return `${getApiBase()}${path}`
}

/** App-owned API base (always points at this app process; never switches with the resource backend). */
export function appApiUrl(path: string) {
  return `${getAppApiBase()}${path}`
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

/** Scope query parameter object. */
export function scopeParams(scope: ScopeRef): Record<string, string | undefined> {
  return scope.type === "agent" ? { scope: "agent", agent_id: scope.agent_id } : { scope: "store" }
}

export function scopeQuery(scope: ScopeRef): string {
  return buildQuery(scopeParams(scope))
}

export async function readJson<T>(response: Response): Promise<T> {
  const text = await response.text()
  const body = text ? JSON.parse(text) : null
  if (!response.ok) {
    const message = body?.message || body?.errors?.[0]?.message || response.statusText
    throw new ApiError(message, response.status, body?.errors?.[0]?.code)
  }
  return body as T
}

async function apiFetch(path: string, init: RequestInit): Promise<Response> {
  const headers = new Headers(init.headers)
  const host = getSelectedHost()
  if (host) headers.set("X-MCPStore-Host", host)
  const token = getPanelToken()
  if (token) headers.set("Authorization", `Bearer ${token}`)
  return fetch(apiUrl(path), { ...init, headers })
}

async function appFetch(path: string, init: RequestInit): Promise<Response> {
  const headers = new Headers(init.headers)
  const token = getPanelToken()
  if (token) headers.set("Authorization", `Bearer ${token}`)
  return fetch(`${getAppApiBase()}${path}`, { ...init, headers })
}

/** Unwraps the one envelope shape the backend produces ({success, data, errors}); non-2xx throws from readJson. */
async function apiAt<T>(
  path: string,
  options: RequestInit,
  fetcher: (path: string, init: RequestInit) => Promise<Response>,
): Promise<T> {
  const headers = new Headers(options.headers)
  headers.set("Accept", "application/json")

  if (
    options.body !== undefined &&
    !(options.body instanceof FormData) &&
    !headers.has("Content-Type")
  ) {
    headers.set("Content-Type", "application/json")
  }

  const response = await fetcher(path, { ...options, headers })
  if (response.status === 401) {
    window.dispatchEvent(new CustomEvent(PANEL_AUTH_REQUIRED_EVENT))
  }
  const payload = await readJson<ApiEnvelope<T>>(response)
  if (!payload.success) {
    throw new ApiError(
      payload.errors?.[0]?.message || payload.message,
      response.status,
      payload.errors?.[0]?.code,
    )
  }
  return payload.data as T
}

/** Resource request: same-origin App face plus the selected host name. */
export function api<T>(path: string, options: RequestInit = {}): Promise<T> {
  return apiAt<T>(path, options, apiFetch)
}

/** Panel-local request: same-origin App face without a resource selector. */
export function appApi<T>(path: string, options: RequestInit = {}): Promise<T> {
  return apiAt<T>(path, options, appFetch)
}
