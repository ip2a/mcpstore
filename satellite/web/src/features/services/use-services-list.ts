import { useQueryClient } from "@tanstack/react-query"
import { useEffect, useMemo, useState } from "react"

import type { ServiceInstance } from "@/lib/api"
import {
  deriveServiceDisplayStatus,
  type ServiceDisplayStatus,
} from "@/features/services/service-display-status"
import { queryKeys } from "@/lib/query-keys"

export type ServiceScopeFilter = "all" | "store" | "agent"
export type ServiceStatusFilter = ServiceDisplayStatus[]
export type ServiceSortDirection = "asc" | "desc"
export type ServiceSortBy = "name" | "status" | "primitives"
export type ServicePrimitiveKind = "tools" | "prompts" | "resources"
export type ServiceSortSpec =
  | { by: "name"; direction: ServiceSortDirection }
  | { by: "primitives"; kind: ServicePrimitiveKind; direction: ServiceSortDirection }
  | { by: "status"; priority: ServiceDisplayStatus[] }

export const SERVICE_STATUS_OPTIONS: ServiceDisplayStatus[] = [
  "connected",
  "connecting",
  "disconnected",
  "error",
]

export const DEFAULT_STATUS_SORT_PRIORITY: ServiceDisplayStatus[] = [
  "connected",
  "connecting",
  "error",
  "disconnected",
]

const ORDER_STORAGE_KEY = "mcpstore.service-list-order"

function readStoredOrder() {
  try {
    const raw = localStorage.getItem(ORDER_STORAGE_KEY)
    const parsed = raw ? JSON.parse(raw) : []
    return Array.isArray(parsed) ? parsed.filter((id): id is string => typeof id === "string") : []
  } catch {
    return []
  }
}

function writeStoredOrder(ids: string[]) {
  try {
    localStorage.setItem(ORDER_STORAGE_KEY, JSON.stringify(ids))
  } catch {
    // Keep the in-memory order when storage is unavailable.
  }
}

function compareName(a: ServiceInstance, b: ServiceInstance) {
  return a.service_name.localeCompare(b.service_name) || a.instance_id.localeCompare(b.instance_id)
}

function statusRank(priority: ServiceDisplayStatus[], status: ServiceDisplayStatus) {
  const index = priority.indexOf(status)
  return index === -1 ? priority.length : index
}

function compareBySort(
  spec: ServiceSortSpec,
  primitiveCount: (service: ServiceInstance, kind: ServicePrimitiveKind) => number,
) {
  return (a: ServiceInstance, b: ServiceInstance) => {
    if (spec.by === "name") {
      const result = compareName(a, b)
      return spec.direction === "asc" ? result : -result
    }
    if (spec.by === "primitives") {
      const delta = primitiveCount(a, spec.kind) - primitiveCount(b, spec.kind)
      const ordered = spec.direction === "asc" ? delta : -delta
      return ordered || compareName(a, b)
    }
    return (
      statusRank(spec.priority, deriveServiceDisplayStatus(a.state)) -
        statusRank(spec.priority, deriveServiceDisplayStatus(b.state)) || compareName(a, b)
    )
  }
}

function sameIds(left: string[], right: string[]) {
  return left.length === right.length && left.every((id, index) => id === right[index])
}

export function reconcileServiceOrder(saved: string[], services: ServiceInstance[]) {
  const ids = new Set(services.map((service) => service.instance_id))
  const kept = saved.filter((id) => ids.has(id))
  const known = new Set(kept)
  const newcomers = services
    .filter((service) => !known.has(service.instance_id))
    .sort(compareName)
    .map((service) => service.instance_id)
  return [...kept, ...newcomers]
}

export function countActiveServiceFilters(filters: {
  scopeFilter: ServiceScopeFilter
  statusFilter: ServiceStatusFilter
}) {
  let count = 0
  if (filters.scopeFilter !== "all") count++
  count += filters.statusFilter.length
  return count
}

export function toggleServiceStatusFilter(
  current: ServiceStatusFilter,
  status: ServiceDisplayStatus,
): ServiceStatusFilter {
  return current.includes(status)
    ? current.filter((item) => item !== status)
    : [...current, status]
}

export function useServicesList(services: ServiceInstance[]) {
  const queryClient = useQueryClient()
  const [query, setQuery] = useState("")
  const [scopeFilter, setScopeFilter] = useState<ServiceScopeFilter>("all")
  const [agentFilter, setAgentFilter] = useState("")
  const [statusFilter, setStatusFilter] = useState<ServiceStatusFilter>([])
  const [order, setOrder] = useState<string[]>(readStoredOrder)
  const resolvedOrder = useMemo(() => reconcileServiceOrder(order, services), [order, services])

  function primitiveCount(service: ServiceInstance, kind: ServicePrimitiveKind) {
    if (kind === "tools") return service.tools?.length || 0
    const cached = queryClient.getQueryData<unknown[]>(
      kind === "prompts"
        ? queryKeys.instancePrompts(service)
        : queryKeys.instanceResources(service),
    )
    return cached?.length ?? 0
  }

  useEffect(() => {
    if (sameIds(resolvedOrder, order)) return
    setOrder(resolvedOrder)
    writeStoredOrder(resolvedOrder)
  }, [order, resolvedOrder])

  const orderedServices = useMemo(() => {
    const byId = new Map(services.map((service) => [service.instance_id, service]))
    return resolvedOrder.flatMap((id) => {
      const service = byId.get(id)
      return service ? [service] : []
    })
  }, [resolvedOrder, services])

  const filteredServices = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase()
    const statusSet = statusFilter.length > 0 ? new Set(statusFilter) : null
    return orderedServices.filter((service) => {
      const description = String(service.effective_config.description || "")
      const scope = service.scope.type === "store" ? "store" : `agent ${service.scope.agent_id}`
      const matchesQuery =
        `${service.service_name} ${service.instance_id} ${scope} ${service.transport} ${description}`
          .toLowerCase()
          .includes(normalizedQuery)
      if (!matchesQuery) return false

      if (scopeFilter === "store" && service.scope.type !== "store") return false
      if (scopeFilter === "agent") {
        if (service.scope.type !== "agent") return false
        if (agentFilter && service.scope.agent_id !== agentFilter) return false
      }

      if (statusSet && !statusSet.has(deriveServiceDisplayStatus(service.state))) return false
      return true
    })
  }, [agentFilter, orderedServices, query, scopeFilter, statusFilter])

  const totals = useMemo(() => {
    return {
      services: filteredServices.length,
      starting: filteredServices.filter((service) => service.state.phase === "starting").length,
    }
  }, [filteredServices])

  const activeFilterCount = countActiveServiceFilters({ scopeFilter, statusFilter })

  const setScopeFilterValue = (value: ServiceScopeFilter) => {
    setScopeFilter(value)
    if (value !== "agent") setAgentFilter("")
  }

  const toggleStatusFilter = (status: ServiceDisplayStatus) => {
    setStatusFilter((current) => toggleServiceStatusFilter(current, status))
  }

  const commitOrder = (next: string[]) => {
    setOrder(next)
    writeStoredOrder(next)
  }

  const applySort = (spec: ServiceSortSpec) => {
    commitOrder(
      [...services]
        .sort(compareBySort(spec, primitiveCount))
        .map((service) => service.instance_id),
    )
  }

  return {
    activeFilterCount,
    agentFilter,
    applySort,
    filteredServices,
    query,
    scopeFilter,
    setAgentFilter,
    setQuery,
    setScopeFilter: setScopeFilterValue,
    setStatusFilter,
    statusFilter,
    toggleStatusFilter,
    totals,
  }
}
