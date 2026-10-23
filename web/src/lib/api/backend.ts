/**
 * Core backend (data source + operation API) base URL.
 *
 * Set from config.toml `[hosts]` when meta loads and after settings save.
 * Can also be initialized from connection hint (injected by daemon's web face).
 * App-owned endpoints always use the same-origin default.
 */

import type { HostsConfig } from "@/lib/api/hosts";
import { resolveActiveHostUrl } from "@/lib/api/hosts";

let activeApiBase: string | null = null;

export function getApiBase(): string {
  if (activeApiBase === null) {
    throw new Error(
      "API base not initialized. Call syncApiBaseFromHosts() first after loading meta."
    );
  }
  return activeApiBase;
}

function setApiBase(url: string): void {
  const trimmed = url.trim();
  if (!trimmed) {
    throw new Error("API base URL cannot be empty");
  }
  activeApiBase = trimmed;
}

export function syncApiBaseFromHosts(hosts: HostsConfig): void {
  setApiBase(resolveActiveHostUrl(hosts));
}

/**
 * Try to initialize API base from connection hint (injected by daemon).
 * This allows web UI to auto-connect to the daemon that served it.
 * Returns true if hint was found and applied.
 */
export function tryInitFromConnectionHint(): boolean {
  const hint = (window as any).__MCPSTORE_CORE_HINT__;
  if (hint?.url && typeof hint.url === "string") {
    try {
      setApiBase(hint.url);
      return true;
    } catch {
      return false;
    }
  }
  return false;
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
