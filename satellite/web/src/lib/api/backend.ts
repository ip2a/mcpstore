/**
 * App-facing API state.
 *
 * The browser and desktop WebView always call the same-origin App face. The
 * selected daemon is identified by name in the request header; URL resolution
 * and failover stay on the backend.
 */

import type { HostsConfig } from "@/lib/api/hosts"

const CORE_HOST_CHANGED = "mcpstore:core-host-changed"

let selectedHost = ""

export function getApiBase(): string {
  return "/api"
}

export function getSelectedHost(): string {
  return selectedHost
}

export function syncApiBaseFromHosts(hosts: HostsConfig): void {
  if (selectedHost === hosts.active) return
  selectedHost = hosts.active
  window.dispatchEvent(new Event(CORE_HOST_CHANGED))
}

/** Connection hints must not override persisted host selection. */
export function tryInitFromConnectionHint(): boolean {
  return false
}

export function onSelectedHostChanged(listener: () => void): () => void {
  window.addEventListener(CORE_HOST_CHANGED, listener)
  return () => window.removeEventListener(CORE_HOST_CHANGED, listener)
}

/** App-owned API base (meta, settings, client-config). Always same-origin. */
export function getAppApiBase(): string {
  return "/api"
}

/** Resolve an api base to an absolute URL (default `/api` -> `origin + /api`). */
export function absoluteApiBase(base: string): string {
  try {
    return new URL(base, window.location.origin).toString()
  } catch {
    return base
  }
}
