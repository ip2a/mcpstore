/**
 * Core backend (data source + operation API) base URL.
 *
 * Stored in localStorage so it stays independent of any backend: the
 * settings page can always open and switch back, even if the selected
 * backend is down. See 架构文档-接口规范v1.md §5 (bootstrap 硬规则).
 */

const STORAGE_KEY = "mcpstore:api-base";
const DEFAULT_API_BASE = "/api";

export function getApiBase(): string {
  try {
    return localStorage.getItem(STORAGE_KEY) || DEFAULT_API_BASE;
  } catch {
    return DEFAULT_API_BASE;
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
