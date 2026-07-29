import { useMemo, useRef, useState, type MutableRefObject } from "react"

import type { ServiceInstance } from "@/lib/api"
import {
  deriveServiceDisplayStatus,
  type ServiceDisplayStatus,
} from "@/features/services/service-display-status"

export type ServiceScopeFilter = "all" | "store" | "agent"
export type ServiceStatusFilter = "all" | ServiceDisplayStatus
export type ServiceSortBy = "name" | "status" | "tools"

const STATUS_SORT_ORDER: Record<ServiceDisplayStatus, number> = {
  connected: 0,
  connecting: 1,
  error: 2,
  disconnected: 3,
}

function compareByStatusThenName(a: ServiceInstance, b: ServiceInstance) {
  const statusDiff =
    STATUS_SORT_ORDER[deriveServiceDisplayStatus(a.state)] -
    STATUS_SORT_ORDER[deriveServiceDisplayStatus(b.state)]
  if (statusDiff !== 0) return statusDiff
  return a.service_name.localeCompare(b.service_name)
}

function buildSessionOrderRank(services: ServiceInstance[]) {
  return new Map(
    [...services]
      .sort(compareByStatusThenName)
      .map((service, index) => [service.instance_id, index] as const),
  )
}

function ensureSessionOrderRank(
  orderRankRef: MutableRefObject<Map<string, number> | null>,
  services: ServiceInstance[],
) {
  if (orderRankRef.current === null && services.length > 0) {
    orderRankRef.current = buildSessionOrderRank(services)
  }
  if (orderRankRef.current === null) {
    orderRankRef.current = new Map()
  }

  let nextRank = orderRankRef.current.size
  for (const service of services) {
    if (!orderRankRef.current.has(service.instance_id)) {
      orderRankRef.current.set(service.instance_id, nextRank++)
    }
  }

  return orderRankRef.current
}

function compareFilteredServices(
  a: ServiceInstance,
  b: ServiceInstance,
  sortBy: ServiceSortBy,
  orderRank: Map<string, number>,
) {
  if (sortBy === "name") {
    return a.service_name.localeCompare(b.service_name)
  }

  if (sortBy === "tools") {
    const toolDiff = (b.tools?.length || 0) - (a.tools?.length || 0)
    return toolDiff || a.service_name.localeCompare(b.service_name)
  }

  const rankA = orderRank.get(a.instance_id) ?? Number.MAX_SAFE_INTEGER
  const rankB = orderRank.get(b.instance_id) ?? Number.MAX_SAFE_INTEGER
  return rankA - rankB || a.service_name.localeCompare(b.service_name)
}

export function countActiveServiceFilters(filters: {
  scopeFilter: ServiceScopeFilter
  statusFilter: ServiceStatusFilter
  sortBy: ServiceSortBy
}) {
  let count = 0
  if (filters.scopeFilter !== "all") count++
  if (filters.statusFilter !== "all") count++
  if (filters.sortBy !== "status") count++
  return count
}

export function useServicesList(services: ServiceInstance[]) {
  const [query, setQuery] = useState("")
  const [scopeFilter, setScopeFilter] = useState<ServiceScopeFilter>("all")
  const [agentFilter, setAgentFilter] = useState("")
  const [statusFilter, setStatusFilter] = useState<ServiceStatusFilter>("all")
  const [sortBy, setSortBy] = useState<ServiceSortBy>("status")
  const sessionOrderRankRef = useRef<Map<string, number> | null>(null)

  const filteredServices = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase()
    const orderRank = ensureSessionOrderRank(sessionOrderRankRef, services)
    const filtered = services.filter((service) => {
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

    return filtered.sort((a, b) => compareFilteredServices(a, b, sortBy, orderRank))
  }, [agentFilter, query, scopeFilter, services, sortBy, statusFilter])

  const totals = useMemo(() => {
    return {
      services: filteredServices.length,
      starting: filteredServices.filter((service) => service.state.phase === "starting").length,
    }
  }, [filteredServices])

  const activeFilterCount = countActiveServiceFilters({ scopeFilter, statusFilter, sortBy })

  const setScopeFilterValue = (value: ServiceScopeFilter) => {
    setScopeFilter(value)
    if (value !== "agent") setAgentFilter("")
  }

  return {
    activeFilterCount,
    agentFilter,
    filteredServices,
    query,
    scopeFilter,
    setAgentFilter,
    setQuery,
    setScopeFilter: setScopeFilterValue,
    setSortBy,
    setStatusFilter,
    sortBy,
    statusFilter,
    totals,
  }
}
