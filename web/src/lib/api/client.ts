import { ApiError, type ApiEnvelope, type CacheBackend, type ScopeRef } from "../api";
import { getApiBase, getAppApiBase } from "./backend";

type FlexibleEnvelope<T> =
  ApiEnvelope<T> | { ok: boolean; message?: string; data?: T; error?: string };

/** Core 后端 base（可切换，默认 /api；指向本地 lib 或远程 mcpstore）。 */
export function apiUrl(path: string) {
  return `${getApiBase()}${path}`;
}

/**
 * App 自有接口 base（固定指向本 app 进程，不随 core 后端切换）。
 * 用于 v1/meta、v1/settings、client-config、aggregate —— 见 接口文档 §附录C。
 */
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

/** 作用域 query 参数对象（文档 §17.2：?scope=store | ?scope=agent&agent_id=…）。 */
export function scopeParams(scope: ScopeRef): Record<string, string | undefined> {
  return scope.type === "agent"
    ? { scope: "agent", agent_id: scope.agent_id }
    : { scope: "store" };
}

/** 作用域 query 串；需附加更多参数时展开 scopeParams(...) 传入 buildQuery。 */
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

async function apiAt<T>(
  path: string,
  options: RequestInit,
  urlFn: (path: string) => string,
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
    await fetch(urlFn(path), {
      ...options,
      headers,
    }),
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

/** Core 接口调用（走可切换的 apiUrl）。 */
export function api<T>(path: string, options: RequestInit = {}): Promise<T> {
  return apiAt<T>(path, options, apiUrl);
}

/** App 自有接口调用（固定走 appApiUrl，不随 core 后端切换）。 */
export function appApi<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  return apiAt<T>(path, options, appApiUrl);
}

async function requestAt<T>(
  path: string,
  init: RequestInit | undefined,
  urlFn: (path: string) => string,
): Promise<T> {
  const response = await fetch(urlFn(path), {
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

/** Core 接口调用（走可切换的 apiUrl）。 */
export function request<T>(path: string, init?: RequestInit): Promise<T> {
  return requestAt<T>(path, init, apiUrl);
}

/** App 自有接口调用（固定走 appApiUrl，不随 core 后端切换）。 */
export function appRequest<T>(path: string, init?: RequestInit): Promise<T> {
  return requestAt<T>(path, init, appApiUrl);
}
