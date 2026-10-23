/**
 * Core backend (data source + operation API) base URL.
 *
 * Set from config.toml `[hosts]` when meta loads and after settings save.
 * App-owned endpoints always use the same-origin default.
 */

import type { HostsConfig } from "@/lib/api/hosts";
import { DEFAULT_API_BASE, resolveActiveHostUrl } from "@/lib/api/hosts";

let activeApiBase = DEFAULT_API_BASE;

export function getApiBase(): string {
  return activeApiBase;
}

function setApiBase(url: string): void {
  const trimmed = url.trim();
  activeApiBase = trimmed || DEFAULT_API_BASE;
}

export function syncApiBaseFromHosts(hosts: HostsConfig): void {
  setApiBase(resolveActiveHostUrl(hosts));
}

/** App-owned API base (meta, settings, client-config). Always same-origin. */
export function getAppApiBase(): string {
  // Web 模式：走 vite proxy 或 daemon web 面的 /api nest
  // Tauri 模式：走内置服务器，默认也用 /api
  return "/api";
}

/** Resolve an api base to an absolute URL (default `/api` -> `origin + /api`). */
export function absoluteApiBase(base: string): string {
  try {
    return new URL(base, window.location.origin).toString();
  } catch {
    return base;
  }
}
