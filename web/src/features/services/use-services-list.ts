import { useEffect, useMemo, useState } from "react"

import type { ServiceInstance } from "@/lib/api"
import {
  deriveServiceDisplayStatus,
  type ServiceDisplayStatus,
} from "@/features/services/service-display-status"

export type ServiceScopeFilter = "all" | "store" | "agent"
export type ServiceStatusFilter = "all" | ServiceDisplayStatus
export type ServiceSortBy = "name" | "status" | "tools"

const ORDER_STORAGE_KEY = "mcpstore.service-list-order"

const STATUS_SORT_ORDER: Record<ServiceDisplayStatus, number> = {
  connected: 0,
  connecting: 1,
  error: 2,
  disconnected: 3,
}

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

function compareBySort(sortBy: ServiceSortBy) {
  return (a: ServiceInstance, b: ServiceInstance) => {
    if (sortBy === "name") return compareName(a, b)
    if (sortBy === "tools") {
      return (b.tools?.length || 0) - (a.tools?.length || 0) || compareName(a, b)
    }
    return (
      STATUS_SORT_ORDER[deriveServiceDisplayStatus(a.state)] -
        STATUS_SORT_ORDER[deriveServiceDisplayStatus(b.state)] || compareName(a, b)
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
  if (filters.statusFilter !== "all") count++
  return count
}

export function useServicesList(services: ServiceInstance[]) {
  const [query, setQuery] = useState("")
  const [scopeFilter, setScopeFilter] = useState<ServiceScopeFilter>("all")
  const [agentFilter, setAgentFilter] = useState("")
  const [statusFilter, setStatusFilter] = useState<ServiceStatusFilter>("all")
  const [order, setOrder] = useState<string[]>(readStoredOrder)
  const resolvedOrder = useMemo(() => reconcileServiceOrder(order, services), [order, services])

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
    return orderedServices.filter((service) => {
      const description = String(service.effective_config.description || "")
      const scope = service.scope.type === "store" ? "store" : `agent ${service.scope.agent_id}`
      const matchesQuery = `${service.service_name} ${service.instance_id} ${scope} ${service.transport} ${description}`
        .toLowerCase()
        .includes(normalizedQuery)
      if (!matchesQuery) return false

      if (scopeFilter === "store" && service.scope.type !== "store") return false
      if (scopeFilter === "agent") {
        if (service.scope.type !== "agent") return false
        if (agentFilter && service.scope.agent_id !== agentFilter) return false
      }

      if (statusFilter !== "all" && deriveServiceDisplayStatus(service.state) !== statusFilter) return false
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

  const commitOrder = (next: string[]) => {
    setOrder(next)
    writeStoredOrder(next)
  }

  const applySort = (sortBy: ServiceSortBy) => {
    commitOrder([...services].sort(compareBySort(sortBy)).map((service) => service.instance_id))
  }

  const moveService = (fromId: string, toId: string) => {
    if (fromId === toId) return
    const next = resolvedOrder.slice()
    const from = next.indexOf(fromId)
    const to = next.indexOf(toId)
    if (from < 0 || to < 0) return
    const [item] = next.splice(from, 1)
    next.splice(to, 0, item)
    commitOrder(next)
  }

  return {
    activeFilterCount,
    agentFilter,
    applySort,
    filteredServices,
    moveService,
    query,
    scopeFilter,
    setAgentFilter,
    setQuery,
    setScopeFilter: setScopeFilterValue,
    setStatusFilter,
    statusFilter,
    totals,
  }
}
