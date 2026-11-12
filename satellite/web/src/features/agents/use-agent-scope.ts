import { useEffect, useMemo, useState } from "react"
import { toast } from "sonner"

import { useScopeServicesQuery } from "@/features/agents/queries"
import type { ServiceInstance, ScopeView } from "@/lib/api"

export type SelectedScope = ScopeView

export function useAgentScope({
  selectedScope,
  services,
}: {
  selectedScope: ScopeView
  services: ServiceInstance[]
}) {
  // Single authority for the grid: one list?scope=… query, with light status polling.
  const scopeServicesQuery = useScopeServicesQuery(selectedScope, {
    refetchInterval: (query) => {
      const rows = query.state.data || []
      const transitioning = rows.some(
        (service) => service.state?.phase === "starting" || service.state?.phase === "stopping",
      )
      return transitioning ? 3_000 : 15_000
    },
  })
  const scopeServices = scopeServicesQuery.data || []

  // Services that can join an agent scope = declared in the store but not yet present under that agent.
  const storeServiceNames = useMemo(
    () => [
      ...new Set(
        services
          .filter((service) => service.scope.type === "store")
          .map((service) => service.service_name),
      ),
    ],
    [services],
  )
  const addableServiceNames = useMemo(() => {
    if (selectedScope.type !== "agent") return []
    return storeServiceNames.filter(
      (name) => !scopeServices.some((service) => service.service_name === name),
    )
  }, [scopeServices, selectedScope, storeServiceNames])
  const [scopeServiceName, setScopeServiceName] = useState("")

  const scopeServicesError = scopeServicesQuery.error
  const scopeServicesErrorMessage =
    scopeServicesError instanceof Error
      ? scopeServicesError.message
      : scopeServicesError
        ? String(scopeServicesError)
        : "Failed to load scope services"
  const loadingScopeServices = scopeServicesQuery.isFetching

  useEffect(() => {
    if (!scopeServiceName || !addableServiceNames.includes(scopeServiceName)) {
      setScopeServiceName(addableServiceNames[0] || "")
    }
  }, [addableServiceNames, scopeServiceName])

  async function loadAgentScope() {
    try {
      const servicesResult = await scopeServicesQuery.refetch()
      if (servicesResult.error) throw servicesResult.error
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to load scope")
    }
  }

  return {
    activeAgentId: selectedScope.type === "agent" ? selectedScope.agent_id : "",
    addableServiceNames,
    loadAgentScope,
    loadingScopeServices,
    scopeServiceName,
    scopeServices,
    scopeServicesError,
    scopeServicesErrorMessage,
    setScopeServiceName,
  }
}
