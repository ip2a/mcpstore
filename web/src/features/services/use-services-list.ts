import { useMemo, useState } from "react"

import type { ServiceInstance } from "@/lib/api"

export function useServicesList(services: ServiceInstance[]) {
  const [query, setQuery] = useState("")
  const filteredServices = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase()
    return services.filter((service) => {
      const description = String(service.effective_config.description || "")
      const scope = service.scope.type === "store" ? "store" : `agent ${service.scope.agent_id}`
      return `${service.service_name} ${service.instance_id} ${scope} ${service.transport} ${description}`
        .toLowerCase()
        .includes(normalizedQuery)
    })
  }, [services, query])
  const totals = useMemo(() => {
    return {
      services: filteredServices.length,
      connecting: filteredServices.filter((service) => service.status === "connecting").length,
    }
  }, [filteredServices])

  return { filteredServices, query, setQuery, totals }
}
