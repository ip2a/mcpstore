import { ApiError, type ApiEnvelope, type CacheBackend, type ScopeRef } from "../api";
import { advanceCoreUrl, getAppApiBase, getApiBase } from "./backend";

// Tauri 环境检测
declare global {
  interface Window {
    __TAURI__?: {
      core: {
        invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
      };
    };
  }
}

type FlexibleEnvelope<T> =
  ApiEnvelope<T> | { ok: boolean; message?: string; data?: T; error?: string };

function apiUrl(path: string) {
  return `${getApiBase()}${path}`;
}

/**
 * App 自有接口 base（固定指向本 app 进程，不随 core 后端切换）。
 * 用于 v1/meta、v1/settings、client-config、mcp-hub —— 见 接口文档 §附录C。
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

/** 检查是否在 Tauri 环境 */
function isTauriEnv(): boolean {
  return typeof window !== "undefined" && !!window.__TAURI__;
}

/** Tauri 环境下的代理请求 */
async function tauriFetch(url: string, options: RequestInit): Promise<Response> {
  if (!window.__TAURI__?.core?.invoke) {
    throw new Error("Tauri invoke not available");
  }

  const urlObj = new URL(url, window.location.origin);
  const path = urlObj.pathname + urlObj.search;
  const targetBase = urlObj.origin;

  // 只有 Core API 走代理（非同源），App API 走本地服务器
  const isAppApi = path.startsWith("/api");
  if (isAppApi) {
    // App API 直接走 fetch（Tauri 内置服务器）
    return fetch(url, options);
  }

  const headers: Record<string, string> = {};
  if (options.headers) {
    if (options.headers instanceof Headers) {
      options.headers.forEach((value, key) => {
        headers[key] = value;
      });
    } else if (Array.isArray(options.headers)) {
      options.headers.forEach(([key, value]) => {
        headers[key] = value;
      });
    } else {
      Object.assign(headers, options.headers);
    }
  }

  const result = (await window.__TAURI__.core.invoke("proxy_api_request", {
    method: options.method || "GET",
    path,
    body: options.body ? String(options.body) : null,
    headers: Object.keys(headers).length > 0 ? headers : null,
    targetBase,
  })) as { status: number; body: string; headers: Record<string, string> };

  return new Response(result.body, {
    status: result.status,
    headers: new Headers(result.headers),
  });
}

function isRetryableMethod(method: string | undefined): boolean {
  const normalized = (method ?? "GET").toUpperCase();
  return normalized === "GET" || normalized === "HEAD" || normalized === "OPTIONS";
}

/**
 * Core 请求统一入口：当前 URL 网络级失败时按 active host 的 urls 顺序降级。
 * 幂等方法（GET/HEAD/OPTIONS）立即在下一候选上重试；其余方法只切换候选、
 * 不自动重试（请求可能已到达对端），由调用方决定是否重发。候选耗尽时抛出
 * 原始错误（明确失败，不静默重试死循环）。
 */
async function coreFetch(path: string, init: RequestInit): Promise<Response> {
  for (;;) {
    try {
      const url = apiUrl(path);
      return await (isTauriEnv() ? tauriFetch(url, init) : fetch(url, init));
    } catch (error) {
      if (error instanceof DOMException && error.name === "AbortError") throw error;
      const hasFallback = advanceCoreUrl();
      if (!isRetryableMethod(init.method) || !hasFallback) throw error;
    }
  }
}

/** App 请求入口：固定同源，无降级。 */
async function appFetch(path: string, init: RequestInit): Promise<Response> {
  const url = `${getAppApiBase()}${path}`;
  return isTauriEnv() ? tauriFetch(url, init) : fetch(url, init);
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
    await fetcher(path, {
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

/** Core 接口调用（走可切换的 coreFetch，含多 URL 降级）。 */
export function api<T>(path: string, options: RequestInit = {}): Promise<T> {
  return apiAt<T>(path, options, coreFetch);
}

/** App 自有接口调用（固定走 appFetch，不随 core 后端切换）。 */
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

/** Core 接口调用（走可切换的 coreFetch，含多 URL 降级）。 */
export function request<T>(path: string, init?: RequestInit): Promise<T> {
  return requestAt<T>(path, init, coreFetch);
}

/** App 自有接口调用（固定走 appFetch，不随 core 后端切换）。 */
export function appRequest<T>(path: string, init?: RequestInit): Promise<T> {
  return requestAt<T>(path, init, appFetch);
}
