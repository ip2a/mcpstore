import type { Query } from "@tanstack/react-query"

import { isServiceTransitioning } from "@/features/services/service-display-status"
import type { ServiceInstance, ServiceState } from "@/lib/api"

type ServiceListQuery = Query<ServiceInstance[], Error, ServiceInstance[], readonly unknown[]>

/** 3s only while something is connecting / disconnecting / recovering; otherwise stop. */
export function serviceListRefetchInterval(query: ServiceListQuery): number | false {
  const rows = query.state.data || []
  return rows.some((service) => isServiceTransitioning(service.state)) ? 3_000 : false
}

/** Page-scoped list options: fresh on enter, poll only while transitioning. */
export const serviceListLiveQueryOptions = {
  refetchOnMount: "always" as const,
  refetchInterval: serviceListRefetchInterval,
}

/** Single-instance status: same policy as the list. */
export function serviceStateRefetchInterval(
  query: Query<ServiceState | null | undefined, Error, ServiceState | null | undefined, readonly unknown[]>,
): number | false {
  return isServiceTransitioning(query.state.data ?? undefined) ? 3_000 : false
}
