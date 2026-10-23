/**
 * Core backend (data source + operation API) base URL.
 *
 * Set from config.toml `[hosts]` when meta loads and after settings save.
 * Can also be initialized from connection hint (injected by daemon's web face).
 * App-owned endpoints always use the same-origin default.
 *
 * The active host may list several candidate URLs (preference order). When the
 * current one fails at the network level, requests advance to the next URL and
 * a background probe returns to the preferred one once it recovers.
 */

import type { HostsConfig } from "@/lib/api/hosts";
import { resolveActiveHostUrls } from "@/lib/api/hosts";

const CORE_URL_CHANGED = "mcpstore:core-url-changed";
const PREFERRED_PROBE_COOLDOWN_MS = 30_000;

let hostUrls: string[] = [];
let urlIndex = 0;
let unreachable = false;
let probeTimer: number | null = null;

export function getApiBase(): string {
  if (hostUrls.length === 0) {
    throw new Error(
      "API base not initialized. Call syncApiBaseFromHosts() first after loading meta."
    );
  }
  return hostUrls[urlIndex];
}

function setUrls(urls: string[]): void {
  if (urls.length === 0) {
    throw new Error("API base URL list cannot be empty");
  }
  // Keep pointing at the current URL while it is still a candidate.
  const current = hostUrls[urlIndex];
  const kept = current ? urls.indexOf(current) : -1;
  hostUrls = urls;
  urlIndex = kept >= 0 ? kept : 0;
  unreachable = false;
  notifyCoreUrlChanged();
}

export function syncApiBaseFromHosts(hosts: HostsConfig): void {
  setUrls(resolveActiveHostUrls(hosts));
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
      setUrls([hint.url]);
      return true;
    } catch {
      return false;
    }
  }
  return false;
}

/**
 * Record that the current URL failed at the network level and move to the
 * next candidate. Returns true when another URL is available for a retry of
 * the current request; false when the list is exhausted (the caller should
 * surface the error). A background probe switches back to the preferred URL
 * once it becomes reachable again.
 */
export function advanceCoreUrl(): boolean {
  if (hostUrls.length === 0) return false;
  if (urlIndex < hostUrls.length - 1) {
    urlIndex += 1;
    unreachable = false;
    notifyCoreUrlChanged();
    schedulePreferredProbe();
    return true;
  }
  if (!unreachable) {
    unreachable = true;
    notifyCoreUrlChanged();
  }
  schedulePreferredProbe();
  return false;
}

export type CoreUrlStatus = {
  url: string;
  /** True when serving from the first (preferred) URL and reachable. */
  primary: boolean;
  unreachable: boolean;
};

export function getCoreUrlStatus(): CoreUrlStatus {
  return {
    url: hostUrls[urlIndex] ?? "",
    primary: urlIndex === 0 && !unreachable,
    unreachable,
  };
}

export function onCoreUrlChanged(listener: () => void): () => void {
  window.addEventListener(CORE_URL_CHANGED, listener);
  return () => window.removeEventListener(CORE_URL_CHANGED, listener);
}

function notifyCoreUrlChanged(): void {
  window.dispatchEvent(new Event(CORE_URL_CHANGED));
}

function schedulePreferredProbe(): void {
  if (probeTimer !== null || hostUrls.length <= 1) return;
  probeTimer = window.setTimeout(() => {
    probeTimer = null;
    void probePreferred();
  }, PREFERRED_PROBE_COOLDOWN_MS);
}

async function probePreferred(): Promise<void> {
  const preferred = hostUrls[0];
  if (preferred === undefined) return;
  try {
    // Any HTTP response proves reachability; only a network failure counts as down.
    await fetch(`${preferred.replace(/\/+$/, "")}/health`);
    if (unreachable || urlIndex !== 0) {
      urlIndex = 0;
      unreachable = false;
      notifyCoreUrlChanged();
    }
  } catch {
    schedulePreferredProbe();
  }
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
