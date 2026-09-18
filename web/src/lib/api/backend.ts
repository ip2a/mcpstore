/**
 * Core backend (data source + operation API) base URL.
 *
 * Runtime value is synced from config.toml `[hosts]` on meta load and after
 * settings save. localStorage keeps the active URL so core API calls survive
 * reloads before meta returns.
 */

import type { HostsConfig } from "@/lib/api/hosts";
import {
  DEFAULT_API_BASE,
  resolveActiveHostUrl,
} from "@/lib/api/hosts";

const STORAGE_KEY = "mcpstore:api-base";

export function getApiBase(): string {
  try {
    return localStorage.getItem(STORAGE_KEY) || DEFAULT_API_BASE;
  } catch {
    return DEFAULT_API_BASE;
  }
}

/** Resolve an api base to an absolute URL (default `/api` -> `origin + /api`). */
export function absoluteApiBase(base: string): string {
  try {
    return new URL(base, window.location.origin).toString();
  } catch {
    return base;
  }
}

export function setApiBase(url: string): void {
  const trimmed = url.trim();
  try {
    if (trimmed && trimmed !== DEFAULT_API_BASE) {
      localStorage.setItem(STORAGE_KEY, trimmed);
    } else {
      localStorage.removeItem(STORAGE_KEY);
    }
  } catch {
    // ignore storage errors (private mode, quota, etc.)
  }
}

export function syncApiBaseFromHosts(hosts: HostsConfig): void {
  setApiBase(resolveActiveHostUrl(hosts));
}

const APP_STORAGE_KEY = "mcpstore:app-api-base";

/**
 * App 自有接口的 base（v1/meta、v1/settings、client-config、aggregate）。
 * 固定指向「本 app 进程」，不随 core 后端切换（getApiBase）变化 —— 见 接口文档 §附录C。
 * 默认 /api（同源，dev 由 Vite proxy 转发到本地 app :1820）。
 */
export function getAppApiBase(): string {
  try {
    return localStorage.getItem(APP_STORAGE_KEY) || DEFAULT_API_BASE;
  } catch {
    return DEFAULT_API_BASE;
  }
}
