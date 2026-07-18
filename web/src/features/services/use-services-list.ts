import { useMemo, useState } from "react"

import type { ReadinessStatus, ServiceInstance } from "@/lib/api"

export type ServiceScopeFilter = "all" | "store" | "agent"
export type ServiceStatusFilter = "all" | ReadinessStatus
export type ServiceSortBy = "name" | "status" | "tools"

const STATUS_SORT_ORDER: Record<ReadinessStatus, number> = {
  ready: 0,
  not_ready: 1,
  unknown: 2,
}

export function countActiveServiceFilters(filters: {
  scopeFilter: ServiceScopeFilter
  statusFilter: ServiceStatusFilter
  sortBy: ServiceSortBy
}) {
  let count = 0
  if (filters.scopeFilter !== "all") count++
  if (filters.statusFilter !== "all") count++
  if (filters.sortBy !== "name") count++
  return count
}

export function useServicesList(services: ServiceInstance[]) {
  const [query, setQuery] = useState("")
  const [scopeFilter, setScopeFilter] = useState<ServiceScopeFilter>("all")
  const [agentFilter, setAgentFilter] = useState("")
  const [statusFilter, setStatusFilter] = useState<ServiceStatusFilter>("all")
  const [sortBy, setSortBy] = useState<ServiceSortBy>("name")

  const filteredServices = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase()
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

      if (statusFilter !== "all" && service.state.readiness.status !== statusFilter) return false
      return true
    })

    return filtered.sort((a, b) => {
      if (sortBy === "status") {
        const statusDiff = STATUS_SORT_ORDER[a.state.readiness.status] - STATUS_SORT_ORDER[b.state.readiness.status]
        return statusDiff || a.service_name.localeCompare(b.service_name)
      }
      if (sortBy === "tools") {
        const toolDiff = (b.tools?.length || 0) - (a.tools?.length || 0)
        return toolDiff || a.service_name.localeCompare(b.service_name)
      }
      return a.service_name.localeCompare(b.service_name)
    })
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
