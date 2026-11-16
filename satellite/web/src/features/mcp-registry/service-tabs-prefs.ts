import {
  deriveServiceDisplayStatus,
  type ServiceDisplayStatus,
} from "@/features/services/service-display-status"
import type { ServiceInstance } from "@/lib/api"

/** Order of service tabs in Tools / Resources / Prompts. */
export type ServiceTabSort = "name" | "added_newest" | "added_oldest"

export type ServiceTabsPrefs = {
  sort: ServiceTabSort
  /** When true, also show configured-but-disconnected services (still hide error/connecting). */
  showDisconnected: boolean
}

export const DEFAULT_SERVICE_TABS_PREFS: ServiceTabsPrefs = {
  sort: "name",
  showDisconnected: false,
}

export function parseServiceTabSort(value: unknown): ServiceTabSort {
  if (value === "name" || value === "added_newest" || value === "added_oldest") return value
  return DEFAULT_SERVICE_TABS_PREFS.sort
}

export function parseServiceTabsPrefs(raw: unknown): ServiceTabsPrefs {
  if (!raw || typeof raw !== "object") return { ...DEFAULT_SERVICE_TABS_PREFS }
  const record = raw as Record<string, unknown>
  return {
    sort: parseServiceTabSort(record.sort),
    showDisconnected: record.show_disconnected === true || record.showDisconnected === true,
  }
}

function isTabVisible(status: ServiceDisplayStatus, showDisconnected: boolean): boolean {
  if (status === "connected") return true
  if (showDisconnected && status === "disconnected") return true
  return false
}

/** Default: ALL − unconnected − error (− connecting). Optional: include disconnected. */
export function filterAndSortScopeInstances(
  instances: ServiceInstance[],
  prefs: ServiceTabsPrefs,
): ServiceInstance[] {
  const filtered = instances.filter((instance) =>
    isTabVisible(deriveServiceDisplayStatus(instance.state), prefs.showDisconnected),
  )

  const sorted = filtered.slice()
  if (prefs.sort === "added_newest") {
    sorted.sort((a, b) => b.added_time - a.added_time || a.service_name.localeCompare(b.service_name))
  } else if (prefs.sort === "added_oldest") {
    sorted.sort((a, b) => a.added_time - b.added_time || a.service_name.localeCompare(b.service_name))
  } else {
    sorted.sort((a, b) => a.service_name.localeCompare(b.service_name))
  }
  return sorted
}
