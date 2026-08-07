import { useEffect, useMemo, useState } from "react"
import { toast } from "sonner"

import { useScopeServicesQuery } from "@/features/agents/queries"
import { useInstanceToolsQueries } from "@/features/tools/queries"
import type { ServiceInstance, ScopeView } from "@/lib/api"

export type SelectedScope = ScopeView

export function useAgentScope({
  busy,
  selectedScope,
  services,
}: {
  busy: string | null
  selectedScope: ScopeView
  services: ServiceInstance[]
}) {
  const scopeServicesQuery = useScopeServicesQuery(selectedScope)
  const scopeServices = scopeServicesQuery.data || []

  // 可加入某 agent scope 的服务 = store 里声明、但该 agent 下还没有的。
  const storeServiceNames = useMemo(
    () =>
      [
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

  const scopeToolQueries = useInstanceToolsQueries(scopeServices, "available")
  const scopeTools = useMemo(
    () =>
      scopeToolQueries.flatMap((result, index) =>
        (result.data || []).map((tool) => ({ instance: scopeServices[index], tool })),
      ),
    [scopeServices, scopeToolQueries],
  )

  const scopeServicesError = scopeServicesQuery.error
  const scopeToolsError =
    scopeToolQueries.find((result) => result.error)?.error || null
  const scopeServicesErrorMessage =
    scopeServicesError instanceof Error
      ? scopeServicesError.message
      : scopeServicesError
        ? String(scopeServicesError)
        : "作用域服务加载失败"
  const scopeToolsErrorMessage =
    scopeToolsError instanceof Error
      ? scopeToolsError.message
      : scopeToolsError
        ? String(scopeToolsError)
        : "作用域工具加载失败"
  const loadingScopeServices = scopeServicesQuery.isFetching
  const loadingScopeTools = scopeToolQueries.some((result) => result.isFetching)

  useEffect(() => {
    if (!scopeServiceName || !addableServiceNames.includes(scopeServiceName)) {
      setScopeServiceName(addableServiceNames[0] || "")
    }
  }, [addableServiceNames, scopeServiceName])

  async function loadAgentScope() {
    try {
      const servicesResult = await scopeServicesQuery.refetch()
      if (servicesResult.error) throw servicesResult.error
      const toolResults = await Promise.all(scopeToolQueries.map((result) => result.refetch()))
      const failed = toolResults.find((result) => result.error)
      if (failed?.error) throw failed.error
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "作用域加载失败")
    }
  }

  useEffect(() => {
    void loadAgentScope()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedScope, busy])

  return {
    activeAgentId: selectedScope.type === "agent" ? selectedScope.agent_id : "",
    addableServiceNames,
    loadAgentScope,
    loadingScopeServices,
    loadingScopeTools,
    scopeServiceName,
    scopeServices,
    scopeServicesError,
    scopeServicesErrorMessage,
    scopeTools,
    scopeToolsError,
    scopeToolsErrorMessage,
    setScopeServiceName,
  }
}
