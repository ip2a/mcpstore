import { ApiError, type ApiEnvelope, type ScopeRef } from "../api";
import { getAppApiBase, getApiBase, getSelectedHost } from "./backend";

type FlexibleEnvelope<T> =
  | ApiEnvelope<T>
  | { ok: boolean; message?: string; data?: T; error?: string };

function apiUrl(path: string) {
  return `${getApiBase()}${path}`;
}

/** App 自有接口 base（固定指向本 app 进程，不随资源后端切换）。 */
export function appApiUrl(path: string) {
  return `${getAppApiBase()}${path}`;
}

export function buildQuery(
  params: Record<string, string | number | boolean | null | undefined>,
) {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value === undefined || value === null || value === "") continue;
    search.set(key, String(value));
  }
  const query = search.toString();
  return query ? `?${query}` : "";
}

/** 作用域 query 参数对象。 */
export function scopeParams(scope: ScopeRef): Record<string, string | undefined> {
  return scope.type === "agent"
    ? { scope: "agent", agent_id: scope.agent_id }
    : { scope: "store" };
}

export function scopeQuery(scope: ScopeRef): string {
  return buildQuery(scopeParams(scope));
}

export async function readJson<T>(response: Response): Promise<T> {
  const text = await response.text();
  const body = text ? JSON.parse(text) : null;
  if (!response.ok) {
    const message =
      body?.message || body?.errors?.[0]?.message || response.statusText;
    throw new ApiError(message, response.status, body?.errors?.[0]?.code);
  }
  return body as T;
}

async function apiFetch(path: string, init: RequestInit): Promise<Response> {
  const headers = new Headers(init.headers);
  const host = getSelectedHost();
  if (host) headers.set("X-MCPStore-Host", host);
  return fetch(apiUrl(path), { ...init, headers });
}

async function appFetch(path: string, init: RequestInit): Promise<Response> {
  return fetch(`${getAppApiBase()}${path}`, init);
}

async function apiAt<T>(
  path: string,
  options: RequestInit,
  fetcher: (path: string, init: RequestInit) => Promise<Response>,
): Promise<T> {
  const headers = new Headers(options.headers);
  headers.set("Accept", "application/json");

  if (
    options.body !== undefined &&
    !(options.body instanceof FormData) &&
    !headers.has("Content-Type")
  ) {
    headers.set("Content-Type", "application/json");
  }

  const payload = await readJson<T | FlexibleEnvelope<T>>(
    await fetcher(path, { ...options, headers }),
  );

  if (payload && typeof payload === "object" && "success" in payload) {
    const envelope = payload as ApiEnvelope<T>;
    if (!envelope.success)
      throw new ApiError(
        envelope.errors?.[0]?.message || envelope.message,
        200,
        envelope.errors?.[0]?.code,
      );
    return envelope.data as T;
  }

  if (payload && typeof payload === "object" && "ok" in payload) {
    const envelope = payload as {
      ok: boolean;
      message?: string;
      data?: T;
      error?: string;
    };
    if (!envelope.ok)
      throw new ApiError(
        envelope.error || envelope.message || "Request failed",
        200,
      );
    return envelope.data as T;
  }

  return payload as T;
}

/** Resource request: same-origin App face plus the selected host name. */
export function api<T>(path: string, options: RequestInit = {}): Promise<T> {
  return apiAt<T>(path, options, apiFetch);
}

/** Panel-local request: same-origin App face without a resource selector. */
export function appApi<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  return apiAt<T>(path, options, appFetch);
}

async function requestAt<T>(
  path: string,
  init: RequestInit | undefined,
  fetcher: (path: string, init: RequestInit) => Promise<Response>,
): Promise<T> {
  const response = await fetcher(path, {
    headers: { "Content-Type": "application/json", ...init?.headers },
    ...init,
  });
  const payload = await readJson<ApiEnvelope<T>>(response);
  if (!payload.success) {
    throw new ApiError(
      payload.errors?.[0]?.message || payload.message,
      response.status,
      payload.errors?.[0]?.code,
    );
  }
  return payload.data as T;
}

export function request<T>(path: string, init?: RequestInit): Promise<T> {
  return requestAt<T>(path, init, apiFetch);
}

export function appRequest<T>(path: string, init?: RequestInit): Promise<T> {
  return requestAt<T>(path, init, appFetch);
}
